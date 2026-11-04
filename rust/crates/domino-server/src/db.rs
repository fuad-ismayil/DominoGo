//! SQLite schema migrations, connection access, and guest cleanup.

use rusqlite::{Connection, Params};
use std::sync::{Mutex, MutexGuard};

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (creating the schema/migrations if needed) exactly like the TS
    /// server startup block.
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// In-memory database for tests.
    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Locked connection with poisoned-lock recovery.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        let conn = self.conn();
        conn.execute_batch(SCHEMA)?;
        // TS wraps each ALTER in try/catch; we only run when the column is absent.
        ensure_column(&conn, "sessions", "expires_at", "DATETIME")?;
        conn.execute(
            "UPDATE sessions SET expires_at = datetime('now', '+30 days') WHERE expires_at IS NULL",
            [],
        )?;
        ensure_column(&conn, "lobbies", "game_mode", "TEXT NOT NULL DEFAULT '101'")?;
        ensure_column(
            &conn,
            "lobbies",
            "draw_rule",
            "TEXT NOT NULL DEFAULT 'anytime'",
        )?;
        ensure_column(&conn, "users", "last_active_at", "DATETIME")?;
        conn.execute(
            "UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE last_active_at IS NULL",
            [],
        )?;
        conn.execute_batch("CREATE TRIGGER IF NOT EXISTS users_initial_activity AFTER INSERT ON users WHEN NEW.last_active_at IS NULL BEGIN UPDATE users SET last_active_at = CURRENT_TIMESTAMP WHERE id = NEW.id; END;")?;
        // Prune expired sessions at startup.
        conn.execute(
            "DELETE FROM sessions WHERE expires_at < datetime('now')",
            [],
        )?;
        Ok(())
    }

    pub fn query_one<T, F>(
        &self,
        sql: &str,
        params: impl Params,
        map: F,
    ) -> rusqlite::Result<Option<T>>
    where
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self.conn();
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params, map)?;
        rows.next().transpose()
    }

    pub fn query_all<T, F>(
        &self,
        sql: &str,
        params: impl Params,
        map: F,
    ) -> rusqlite::Result<Vec<T>>
    where
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self.conn();
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, map)?;
        rows.collect()
    }

    pub fn execute(&self, sql: &str, params: impl Params) -> rusqlite::Result<usize> {
        self.conn().execute(sql, params)
    }

    /// INSERT (or other statement) followed by `last_insert_rowid()` under the
    /// SAME connection lock — DB-level inserts from concurrent sockets could
    /// otherwise read a foreign rowid.
    pub fn execute_with_rowid(&self, sql: &str, params: impl Params) -> rusqlite::Result<i64> {
        let conn = self.conn();
        let changes = conn.execute(sql, params)?;
        let _ = changes;
        Ok(conn.last_insert_rowid())
    }

    /// `cleanupStaleGuests` — the exact seven statements from `server.ts`,
    /// deleting guests whose `last_active_at` is ≥24 h old.
    pub fn cleanup_stale_guests(&self) -> rusqlite::Result<u64> {
        let cond = "SELECT id FROM users WHERE is_guest = 1 AND id NOT LIKE 'ai_%' AND last_active_at < datetime('now', '-24 hours') AND id NOT IN (SELECT lp.user_id FROM lobby_players lp JOIN lobbies l ON l.id = lp.lobby_id WHERE l.status IN ('playing', 'stopped'))";
        const STMTS: [&str; 7] = [
            "DELETE FROM sessions WHERE user_id IN (%COND%)",
            "DELETE FROM lobby_players WHERE user_id IN (%COND%)",
            "DELETE FROM lobbies WHERE creator_id IN (%COND%)",
            "DELETE FROM friendships WHERE sender_id IN (%COND%) OR receiver_id IN (%COND%)",
            "DELETE FROM game_invitations WHERE sender_id IN (%COND%) OR receiver_id IN (%COND%)",
            "DELETE FROM player_stats WHERE user_id IN (%COND%)",
            "DELETE FROM users WHERE id IN (%COND%)",
        ];
        let conn = self.conn();
        let mut changes = 0u64;
        for stmt in STMTS {
            let sql = stmt.replace("%COND%", cond);
            changes += conn.execute(&sql, [])? as u64;
        }
        Ok(changes)
    }
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    decl: &str,
) -> rusqlite::Result<bool> {
    let exists = {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = stmt.query([])?;
        let mut found = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                found = true;
                break;
            }
        }
        found
    };
    if !exists {
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
    }
    Ok(exists)
}

/// Exact schema from `server.ts`. AI bot users seeded identically.
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    is_guest BOOLEAN NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS lobbies (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    max_players INTEGER NOT NULL,
    status TEXT DEFAULT 'waiting',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS lobby_players (
    lobby_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    joined_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (lobby_id, user_id),
    FOREIGN KEY (lobby_id) REFERENCES lobbies (id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS game_states (
    lobby_id TEXT PRIMARY KEY,
    state_json TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (lobby_id) REFERENCES lobbies (id) ON DELETE CASCADE
);

INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_1', 'AI Bot 1', 1);
INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_2', 'AI Bot 2', 1);
INSERT OR IGNORE INTO users (id, username, is_guest) VALUES ('ai_3', 'AI Bot 3', 1);

CREATE TABLE IF NOT EXISTS friendships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sender_id TEXT NOT NULL,
    receiver_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(sender_id, receiver_id)
);

CREATE TABLE IF NOT EXISTS game_invitations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lobby_id TEXT NOT NULL,
    sender_id TEXT NOT NULL,
    receiver_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (lobby_id) REFERENCES lobbies(id) ON DELETE CASCADE,
    FOREIGN KEY (sender_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (receiver_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ranking_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT OR IGNORE INTO ranking_metadata(key,value)
VALUES ('tracking_started_at', strftime('%Y-%m-%dT%H:%M:%SZ','now'));
CREATE TABLE IF NOT EXISTS leaderboard_match_completions (
    match_id TEXT PRIMARY KEY,
    completed_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS dated_player_stats (
    match_id TEXT NOT NULL REFERENCES leaderboard_match_completions(match_id),
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wins INTEGER NOT NULL,
    losses INTEGER NOT NULL,
    total_score INTEGER NOT NULL,
    PRIMARY KEY(match_id,user_id)
);
CREATE INDEX IF NOT EXISTS leaderboard_match_completion_time ON leaderboard_match_completions(completed_at_ms,match_id);
CREATE INDEX IF NOT EXISTS dated_player_stats_user ON dated_player_stats(user_id,match_id);

CREATE TABLE IF NOT EXISTS player_stats (
    user_id TEXT PRIMARY KEY,
    wins INTEGER NOT NULL DEFAULT 0,
    losses INTEGER NOT NULL DEFAULT 0,
    total_score INTEGER NOT NULL DEFAULT 0,
    games_played INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS completed_matches (
    id TEXT PRIMARY KEY,
    lobby_id TEXT NOT NULL,
    summary_json TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS completed_match_players (
    match_id TEXT NOT NULL REFERENCES completed_matches(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    hidden INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (match_id, user_id)
);

CREATE TABLE IF NOT EXISTS hidden_match_history (
    user_id TEXT NOT NULL,
    lobby_id TEXT NOT NULL,
    hidden_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, lobby_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (lobby_id) REFERENCES lobbies(id) ON DELETE CASCADE
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_and_seeding_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db
            .query_one("SELECT COUNT(*) FROM users WHERE id LIKE 'ai_%'", [], |r| {
                r.get(0)
            })
            .unwrap()
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn migrations_are_idempotent() {
        let db = Db::open_in_memory().unwrap();
        // Opening again on the same file must not fail.
        let conn = db.conn();
        let expires = conn
            .prepare("PRAGMA table_info(sessions)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(Result::ok)
            .any(|c| c == "expires_at");
        assert!(expires);
    }
    #[test]
    fn dated_tracking_migration_preserves_existing_lifetime_stats() {
        let file = tempfile::NamedTempFile::new().unwrap();
        {
            let conn = Connection::open(file.path()).unwrap();
            conn.execute_batch("CREATE TABLE users(id TEXT PRIMARY KEY,username TEXT UNIQUE NOT NULL,password_hash TEXT,is_guest BOOLEAN NOT NULL DEFAULT 0,created_at DATETIME DEFAULT CURRENT_TIMESTAMP); CREATE TABLE player_stats(user_id TEXT PRIMARY KEY,wins INTEGER NOT NULL DEFAULT 0,losses INTEGER NOT NULL DEFAULT 0,total_score INTEGER NOT NULL DEFAULT 0,games_played INTEGER NOT NULL DEFAULT 0,FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE); INSERT INTO users(id,username) VALUES ('legacy','Legacy'); INSERT INTO player_stats VALUES ('legacy',12,4,1700,16);").unwrap();
        }
        let path = file.path().to_str().unwrap();
        let db = Db::open(path).unwrap();
        let totals=db.query_one("SELECT wins,losses,total_score,games_played FROM player_stats WHERE user_id='legacy'",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?))).unwrap();
        assert_eq!(totals, Some((12, 4, 1700, 16)));
        let started: String = db
            .query_one(
                "SELECT value FROM ranking_metadata WHERE key='tracking_started_at'",
                [],
                |r| r.get(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            db.query_one("SELECT COUNT(*) FROM dated_player_stats", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            Some(0)
        );
        drop(db);
        let reopened = Db::open(path).unwrap();
        assert_eq!(
            reopened
                .query_one(
                    "SELECT value FROM ranking_metadata WHERE key='tracking_started_at'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            Some(started)
        );
        assert_eq!(
            reopened
                .query_one(
                    "SELECT total_score FROM player_stats WHERE user_id='legacy'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            Some(1700)
        );
    }
}
