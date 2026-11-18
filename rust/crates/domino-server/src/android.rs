//! JNI entry points. Android runs this in a dedicated foreground-service process.
use jni::{
    objects::{JClass, JString},
    sys::jstring,
    JNIEnv,
};
use std::sync::{Arc, OnceLock};
use tokio::sync::Notify;

static STOP: OnceLock<Notify> = OnceLock::new();
static CONTROL: OnceLock<(tokio::runtime::Handle, Arc<crate::domain::AppState>)> = OnceLock::new();
pub fn install_control(app: Arc<crate::domain::AppState>) {
    let _ = CONTROL.set((tokio::runtime::Handle::current(), app));
}

#[no_mangle]
pub extern "system" fn Java_games_dominogo_host_HostService_nativeSetEnabled(
    mut env: JNIEnv,
    _class: JClass,
    game: JString,
    enabled: jni::sys::jboolean,
) -> jstring {
    let result = (|| -> Result<(), String> {
        let name: String = env.get_string(&game).map_err(|e| e.to_string())?.into();
        let game = crate::lifecycle::GameId::parse(&name).ok_or("Unknown game")?;
        let (runtime, app) = CONTROL.get().ok_or("Host is still starting")?;
        let app = app.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        runtime.spawn(async move {
            let _ = tx.send(crate::lifecycle::set_enabled(&app, game, enabled != 0).await);
        });
        rx.recv_timeout(std::time::Duration::from_secs(15))
            .map_err(|_| "Host control timed out".to_string())?
    })();
    env.new_string(result.err().unwrap_or_default())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "system" fn Java_games_dominogo_host_HostService_nativeStatus(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let status = CONTROL
        .get()
        .map(|(_, app)| {
            let mut status = app.games.status();
            status["hostId"] = serde_json::json!(app.host_id);
            status.to_string()
        })
        .unwrap_or_else(|| "{}".into());
    env.new_string(status)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

pub async fn shutdown() {
    STOP.get_or_init(Notify::new).notified().await;
}

#[no_mangle]
pub extern "system" fn Java_games_dominogo_host_HostService_nativeStop(
    _env: JNIEnv,
    _class: JClass,
) {
    STOP.get_or_init(Notify::new).notify_one();
}

#[no_mangle]
pub extern "system" fn Java_games_dominogo_host_HostService_nativeRun(
    mut env: JNIEnv,
    _class: JClass,
    db: JString,
    web: JString,
    uno_web: JString,
) -> jstring {
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let db: String = env.get_string(&db).map_err(|e| e.to_string())?.into();
            let web: String = env.get_string(&web).map_err(|e| e.to_string())?.into();
            let uno_web: String = env.get_string(&uno_web).map_err(|e| e.to_string())?.into();
            // Set before creating the server runtime; there is one server per host process.
            std::env::set_var("DOMINO_DB_PATH", db);
            std::env::set_var("DOMINO_STATIC_DIR", web);
            std::env::set_var("DOMINO_PORT", "3000");
            std::env::set_var("UNOGO_PORT", "3001");
            std::env::set_var("UNOGO_STATIC_DIR", uno_web);
            std::env::set_var("GO_HOST_INITIAL_GAMES", "none");
            let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
            runtime.block_on(crate::run());
            Ok(())
        }));
    let message = match result {
        Ok(Ok(())) => String::new(),
        Ok(Err(e)) => e,
        Err(e) => e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|v| v.to_string()))
            .unwrap_or_else(|| "Server stopped unexpectedly".into()),
    };
    env.new_string(message)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_games_dominogo_host_SyncReceiver_nativeDatabase(
    mut env: JNIEnv,
    _class: JClass,
    directory: JString,
    operation: JString,
) -> jstring {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let directory: String = env.get_string(&directory)?.into();
        let operation: String = env.get_string(&operation)?.into();
        database_operation(std::path::Path::new(&directory), &operation)
    })();
    let message = result.err().map(|e| e.to_string()).unwrap_or_default();
    env.new_string(message)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn database_operation(
    dir: &std::path::Path,
    operation: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use rusqlite::{backup::Backup, Connection};
    let database = dir.join("database.sqlite");
    let baseline = dir.join("sync-before.sqlite");
    let incoming = dir.join("sync-incoming.sqlite");
    if operation == "export" {
        if !database.exists() {
            return Err("Phone database has not been seeded yet".into());
        }
        // An updated APK may be synced before its first Start. Apply the shared
        // additive migration under SyncReceiver's exclusive database lock.
        drop(crate::db::Db::open(database.to_string_lossy().as_ref())?);
        let source = Connection::open(&database)?;
        let _ = std::fs::remove_file(&baseline);
        let mut output = Connection::open(&baseline)?;
        Backup::new(&source, &mut output)?.run_to_completion(
            128,
            std::time::Duration::from_millis(10),
            None,
        )?;
        return Ok(());
    }
    if operation != "import" && operation != "seed" {
        return Err("Unknown operation".into());
    }
    if !incoming.exists() {
        return Err("Incoming database missing".into());
    }
    if operation == "seed" && database.exists() {
        return Err("Phone already has a database; use merge sync".into());
    }
    let source =
        Connection::open_with_flags(&incoming, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err("Incoming database failed integrity check".into());
    }
    let violations: i64 =
        source.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
    if violations != 0 {
        return Err("Incoming database has broken references".into());
    }
    let mut target = Connection::open(&database)?;
    if operation == "import" {
        target.execute(
            "ATTACH DATABASE ? AS baseline",
            [baseline.to_string_lossy().as_ref()],
        )?;
        let tables: Vec<String> = target.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
            .query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for table in tables {
            let name = table.replace('"', "\"\"");
            let changed: bool = target.query_row(&format!("SELECT EXISTS(SELECT * FROM main.\"{name}\" EXCEPT SELECT * FROM baseline.\"{name}\") OR EXISTS(SELECT * FROM baseline.\"{name}\" EXCEPT SELECT * FROM main.\"{name}\")"), [], |r| r.get(0))?;
            if changed {
                return Err("Phone database changed during sync; retry".into());
            }
        }
        target.execute_batch("DETACH DATABASE baseline;")?;
    }
    // Keep a private recovery copy before changing any phone data.
    if operation == "import" {
        let mut recovery = Connection::open(dir.join("database-before-sync.sqlite"))?;
        Backup::new(&target, &mut recovery)?.run_to_completion(
            128,
            std::time::Duration::from_millis(10),
            None,
        )?;
    }
    Backup::new(&source, &mut target)?.run_to_completion(
        128,
        std::time::Duration::from_millis(10),
        None,
    )?;
    Ok(())
}
