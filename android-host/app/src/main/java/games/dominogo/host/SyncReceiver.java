package games.dominogo.host;

import android.app.ActivityManager;
import android.content.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** Private-file sync bridge, accessible only to the Android shell/system DUMP permission. */
public class SyncReceiver extends BroadcastReceiver {
    private static native String nativeDatabase(String directory, String operation);
    public void onReceive(Context context, Intent intent) {
        PendingResult pending = goAsync();
        String operation = intent.getStringExtra("operation");
        String request = intent.getStringExtra("request");
        new Thread(() -> {
            String error = "";
            try (RandomAccessFile lockFile = new RandomAccessFile(new File(context.getFilesDir(), "database.lock"), "rw");
                 java.nio.channels.FileLock lock = lockFile.getChannel().tryLock()) {
                if (lock == null) throw new IOException("Stop the phone server before syncing");
                if (!"export".equals(operation) && !"import".equals(operation) && !"seed".equals(operation) && !"configure".equals(operation)) throw new IOException("Unknown sync operation");
                for (ActivityManager.RunningAppProcessInfo p : context.getSystemService(ActivityManager.class).getRunningAppProcesses()) {
                    if (p.processName.equals(context.getPackageName() + ":host")) throw new IOException("Stop the phone server before syncing");
                }
                if ("configure".equals(operation)) {
                    File config = new File(context.getFilesDir(), "hosting-config.json");
                    JSONObject values = new JSONObject(new String(Files.readAllBytes(config.toPath()), StandardCharsets.UTF_8));
                    String hostname = values.getString("hostname"), token = values.getString("token");
                    java.net.URI uri = new java.net.URI(hostname);
                    if (!"https".equals(uri.getScheme()) || uri.getHost() == null || !uri.getPath().isEmpty() || token.isEmpty()) throw new IOException("Invalid tunnel settings");
                    android.content.SharedPreferences.Editor settings=context.getSharedPreferences("hosting", Context.MODE_PRIVATE).edit().putString("hostname",hostname).putString("token",token);
                    if(values.has("unoHostname")){
                        String uno=values.getString("unoHostname");java.net.URI unoUri=new java.net.URI(uno);
                        if(!"https".equals(unoUri.getScheme())||unoUri.getHost()==null||unoUri.getUserInfo()!=null||unoUri.getQuery()!=null||unoUri.getFragment()!=null||!unoUri.getPath().isEmpty())throw new IOException("Invalid UnoGo hostname");
                        settings.putString("unoHostname",uno);
                    }
                    if (!settings.commit()) throw new IOException("Cannot save tunnel settings");
                    Files.delete(config.toPath());
                } else {
                    System.loadLibrary("domino_server");
                    error = nativeDatabase(context.getFilesDir().getAbsolutePath(), operation);
                    if (error == null) error = "Database operation failed";
                }
            } catch (Throwable e) { error = e.getMessage() == null ? e.toString() : e.getMessage(); }
            try {
                JSONObject result = new JSONObject().put("request", request).put("operation", operation)
                    .put("ok", error.isEmpty()).put("message", error.isEmpty() ? (operation.equals("export") ? "Ready to sync" : "Database synced with PC") : error)
                    .put("updated", System.currentTimeMillis());
                Files.write(new File(context.getFilesDir(), "sync-result.json").toPath(), result.toString().getBytes(StandardCharsets.UTF_8));
            } catch (Exception ignored) { }
            pending.finish();
        }, "database-sync").start();
    }
}
