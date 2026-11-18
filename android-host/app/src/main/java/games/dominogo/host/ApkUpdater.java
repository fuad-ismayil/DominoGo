package games.dominogo.host;

import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.content.pm.PackageInstaller;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.os.Build;
import org.json.JSONObject;
import java.io.*;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/**
 * Downloads a published Go Host APK, proves it is the file GitHub published
 * (SHA-256) and that it comes from the same signer as this installation, then
 * hands it to Android's PackageInstaller. Android itself still refuses any APK
 * whose signature differs; the checks here fail earlier with a clear message.
 */
final class ApkUpdater {
    static final String RESUME_FILE = "resume-after-update.json";
    private static final long MAX_APK_BYTES = 200L * 1024 * 1024;

    interface Progress { void update(long done, long total); }

    private ApkUpdater() { }

    /** Download and verify. Returns the verified APK; throws with a user-facing message. */
    static File download(Context context, GitHubUpdates.Result update, Progress progress) throws Exception {
        if (!update.installable()) throw new IOException("This release has no verifiable APK.");
        String expected = GitHubUpdates.parseChecksum(new String(fetch(update.checksumUrl, 1024, null, null), StandardCharsets.UTF_8));
        File dir = new File(context.getCacheDir(), "updates");
        if (!dir.isDirectory() && !dir.mkdirs()) throw new IOException("Could not prepare update storage.");
        for (File old : dir.listFiles() == null ? new File[0] : dir.listFiles()) old.delete();
        File part = new File(dir, "GoHost-" + update.version + ".apk.part");
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (OutputStream out = new BufferedOutputStream(new FileOutputStream(part))) {
            fetch(update.url, MAX_APK_BYTES, out, (done, total) -> progress.update(done, total), digest);
        }
        String actual = hex(digest.digest());
        if (!actual.equals(expected)) {
            part.delete();
            throw new IOException("The downloaded APK does not match its published SHA-256. Nothing was installed.");
        }
        File apk = new File(dir, "GoHost-" + update.version + ".apk");
        if (!part.renameTo(apk)) throw new IOException("Could not finish the download.");
        verifyPackage(context, apk);
        return apk;
    }

    /** Same package, newer version, same signing certificate as this installation. */
    @SuppressWarnings("deprecation")
    static void verifyPackage(Context context, File apk) throws Exception {
        PackageManager pm = context.getPackageManager();
        int flags = Build.VERSION.SDK_INT >= 28 ? PackageManager.GET_SIGNING_CERTIFICATES : PackageManager.GET_SIGNATURES;
        PackageInfo candidate = pm.getPackageArchiveInfo(apk.getAbsolutePath(), flags);
        if (candidate == null) throw new IOException("The downloaded file is not a valid APK.");
        if (!context.getPackageName().equals(candidate.packageName)) throw new IOException("The APK belongs to a different app.");
        PackageInfo installed = pm.getPackageInfo(context.getPackageName(), flags);
        long newCode = Build.VERSION.SDK_INT >= 28 ? candidate.getLongVersionCode() : candidate.versionCode;
        long oldCode = Build.VERSION.SDK_INT >= 28 ? installed.getLongVersionCode() : installed.versionCode;
        if (newCode <= oldCode) throw new IOException("The APK is not newer than the installed app.");
        Set<String> want = signers(installed), got = signers(candidate);
        // Some Android versions omit archive signers; the installer enforces them regardless.
        if (!got.isEmpty() && !want.equals(got)) throw new IOException("The APK is signed with a different key. Nothing was installed.");
    }

    @SuppressWarnings("deprecation")
    private static Set<String> signers(PackageInfo info) throws Exception {
        Signature[] list;
        if (Build.VERSION.SDK_INT >= 28) list = info.signingInfo == null ? null : info.signingInfo.getApkContentsSigners();
        else list = info.signatures;
        Set<String> out = new HashSet<>();
        if (list != null) for (Signature s : list) out.add(hex(MessageDigest.getInstance("SHA-256").digest(s.toByteArray())));
        return out;
    }

    /** Commit the verified APK. The result arrives at {@link UpdateReceiver}. */
    static void install(Context context, File apk) throws IOException {
        PackageInstaller installer = context.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(context.getPackageName());
        params.setSize(apk.length());
        // Once Go Host installed itself, Android 12+ can apply later updates without a prompt.
        if (Build.VERSION.SDK_INT >= 31) params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED);
        if (Build.VERSION.SDK_INT >= 34) params.setRequestUpdateOwnership(true);
        int id = installer.createSession(params);
        try (PackageInstaller.Session session = installer.openSession(id)) {
            try (InputStream in = new FileInputStream(apk); OutputStream out = session.openWrite("GoHost.apk", 0, apk.length())) {
                byte[] buffer = new byte[64 * 1024]; int n;
                while ((n = in.read(buffer)) > 0) out.write(buffer, 0, n);
                session.fsync(out);
            }
            Intent result = new Intent(context, UpdateReceiver.class).setAction(UpdateReceiver.INSTALL_RESULT);
            int mutable = Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0;
            PendingIntent callback = PendingIntent.getBroadcast(context, id, result, PendingIntent.FLAG_UPDATE_CURRENT | mutable);
            session.commit(callback.getIntentSender());
        } catch (IOException | RuntimeException e) {
            installer.abandonSession(id);
            throw e;
        }
    }

    /** Remember which games to start again once the new version is running. */
    static void rememberResume(Context context, String target, String version) {
        try {
            JSONObject plan = new JSONObject().put("target", target).put("version", version).put("at", System.currentTimeMillis());
            Files.write(new File(context.getFilesDir(), RESUME_FILE).toPath(), plan.toString().getBytes(StandardCharsets.UTF_8));
        } catch (Exception ignored) { }
    }

    /** Reads and deletes the resume plan; plans older than 15 minutes are ignored. */
    static String takeResume(Context context) {
        File file = new File(context.getFilesDir(), RESUME_FILE);
        try {
            if (!file.isFile()) return "";
            JSONObject plan = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
            boolean fresh = System.currentTimeMillis() - plan.optLong("at") < 15 * 60_000L;
            String target = plan.optString("target");
            return fresh && Arrays.asList("both", "dominogo", "unogo").contains(target) ? target : "";
        } catch (Exception e) {
            return "";
        } finally {
            file.delete();
        }
    }

    static void writeState(Context context, String message, boolean failed) {
        try {
            JSONObject state = new JSONObject().put("failed", failed).put("news", false).put("message", message)
                .put("url", "").put("revision", "").put("checked", System.currentTimeMillis());
            Files.write(new File(context.getFilesDir(), "update-state.json").toPath(), state.toString().getBytes(StandardCharsets.UTF_8));
        } catch (Exception ignored) { }
    }

    private static byte[] fetch(String url, long limit, OutputStream sink, Progress progress) throws Exception {
        return fetch(url, limit, sink, progress, null);
    }

    private static byte[] fetch(String url, long limit, OutputStream sink, Progress progress, MessageDigest digest) throws Exception {
        if (!url.startsWith(GitHubUpdates.REPOSITORY + "/releases/download/")) throw new IOException("Unexpected download address.");
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        try {
            c.setInstanceFollowRedirects(true);
            c.setConnectTimeout(10_000); c.setReadTimeout(20_000);
            c.setRequestProperty("User-Agent", "DominoGo-Host/" + BuildConfig.VERSION_NAME);
            int code = c.getResponseCode();
            if (code != 200) throw new IOException("GitHub download failed (HTTP " + code + ").");
            if (!"https".equals(c.getURL().getProtocol())) throw new IOException("Refusing an insecure download.");
            long total = c.getContentLengthLong();
            if (total > limit) throw new IOException("The download is larger than expected.");
            ByteArrayOutputStream memory = sink == null ? new ByteArrayOutputStream() : null;
            OutputStream out = sink == null ? memory : sink;
            try (InputStream in = c.getInputStream()) {
                byte[] buffer = new byte[64 * 1024]; long done = 0; int n;
                while ((n = in.read(buffer)) > 0) {
                    if (Thread.currentThread().isInterrupted()) throw new InterruptedIOException("Update canceled.");
                    done += n;
                    if (done > limit) throw new IOException("The download is larger than expected.");
                    out.write(buffer, 0, n);
                    if (digest != null) digest.update(buffer, 0, n);
                    if (progress != null) progress.update(done, total);
                }
            }
            return memory == null ? new byte[0] : memory.toByteArray();
        } finally { c.disconnect(); }
    }

    static String hex(byte[] bytes) {
        StringBuilder s = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) s.append(String.format("%02x", b));
        return s.toString();
    }
}
