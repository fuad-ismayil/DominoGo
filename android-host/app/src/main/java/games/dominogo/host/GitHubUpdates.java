package games.dominogo.host;

import org.json.JSONArray;
import org.json.JSONObject;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.regex.*;

/** Checks published source changes and installable Android releases, never executes source code. */
final class GitHubUpdates {
    static final String REPOSITORY = "https://github.com/fuad-ismayil/DominoGo";
    static final String LATEST_COMMIT = "/commits?sha=main&per_page=1";
    private static final String API = "https://api.github.com/repos/fuad-ismayil/DominoGo";
    interface Fetcher { JSONObject get(String path, boolean allowMissing) throws Exception; }

    /** GitHub's anonymous API allows 60 requests per hour per public IP address. */
    static final class RateLimited extends IOException {
        final long resetAt;
        RateLimited(long resetAt) { super("GitHub is limiting update checks right now."); this.resetAt = resetAt; }
    }

    static final class Result {
        final boolean failed, news;
        /** The check was skipped because GitHub rate-limited it; hosting should not wait for it. */
        boolean limited;
        final String message, url, revision;
        /** Set only for a newer APK release that also publishes GoHost.apk.sha256. */
        final String version, checksumUrl;
        final long checked;
        Result(boolean failed, boolean news, String message, String url, String revision, long checked) {
            this(failed, news, message, url, revision, checked, "", "");
        }
        Result(boolean failed, boolean news, String message, String url, String revision, long checked, String version, String checksumUrl) {
            this.failed = failed; this.news = news; this.message = message;
            this.url = url; this.revision = revision; this.checked = checked;
            this.version = version; this.checksumUrl = checksumUrl;
        }
        /** True when the app can download, verify and install this update itself. */
        boolean installable() { return news && !version.isEmpty() && !checksumUrl.isEmpty() && url.endsWith("/GoHost.apk"); }
        JSONObject json() throws Exception {
            return new JSONObject().put("failed", failed).put("news", news).put("message", message)
                .put("url", url).put("revision", revision).put("checked", checked)
                .put("version", version).put("checksumUrl", checksumUrl);
        }
    }

    private final Fetcher fetcher;
    private final String installedRevision, installedVersion;
    /** cacheDir keeps ETags and answers; GitHub does not count "not modified" replies. */
    GitHubUpdates(File cacheDir) { this(new Network(cacheDir), BuildConfig.SOURCE_REVISION, BuildConfig.VERSION_NAME); }
    GitHubUpdates(Fetcher fetcher, String revision, String version) {
        this.fetcher = fetcher; this.installedRevision = revision; this.installedVersion = version;
    }

    Result check() {
        long checked = System.currentTimeMillis();
        try {
            JSONObject latest = fetcher.get(LATEST_COMMIT, false);
            String sha = latest.getString("sha");
            if (!sha.matches("[0-9a-f]{40}")) throw new IOException("GitHub returned an invalid revision.");
            JSONObject release = fetcher.get("/releases/latest", true);
            String androidRelease = newerAndroidRelease(release, installedVersion);
            if (!androidRelease.isEmpty()) {
                String notes = shortText(release.optString("body"), 700);
                String url = verifiedAsset(release);
                return new Result(false, true, "Go Host " + androidRelease + " is available. Installed: " + installedVersion + "."
                    + (notes.isEmpty() ? "" : "\n\n" + notes), url, sha, checked, androidRelease, checksumAsset(release));
            }
            if (!installedRevision.matches("[0-9a-f]{40}")) {
                // Local builds intentionally omit a published source revision.
                // The release check above still runs and still offers verified APKs.
                // Missing comparison metadata is not a failed network check.
                return new Result(false, false, "GitHub checked for app updates. No newer Android app is published. You are running locally built version " + installedVersion + ".", "", sha, checked);
            }
            if (!sha.equals(installedRevision)) {
                JSONObject comparison;
                try { comparison = fetcher.get("/compare/" + installedRevision + "..." + sha + "?per_page=1", true); }
                catch (IOException e) { comparison = null; }
                String status = comparison == null ? "unknown" : comparison.optString("status");
                if (status.equals("behind")) return new Result(false,true,"GitHub main is behind this installed build. Review the comparison; no downgrade will be installed.",REPOSITORY+"/compare/"+installedRevision+"..."+sha,sha,checked);
                if (!status.equals("behind") && !status.equals("identical")) {
                    JSONObject commit = latest.optJSONObject("commit");
                    String title = commit == null ? "" : shortText(commit.optString("message").split("\n", 2)[0], 180);
                    String prefix = comparison != null && status.equals("ahead") ? comparison.optInt("ahead_by") + " new GitHub commit(s)." : "The GitHub source has changed.";
                    return new Result(false, true, prefix + (title.isEmpty() ? "" : "\n\nLatest: " + title)
                        + "\n\nNo newer Android APK release is published yet. Source changes need a rebuilt APK before this phone can run them.",
                        REPOSITORY + "/compare/" + installedRevision + "..." + sha, sha, checked);
                }
            }
            return new Result(false, false, "No newer published changes found. Installed version " + installedVersion + ".", "", sha, checked);
        } catch (RateLimited e) {
            String when = e.resetAt <= 0 ? "within the hour" : "after " + new java.text.SimpleDateFormat("HH:mm", java.util.Locale.ROOT).format(new java.util.Date(e.resetAt));
            Result skipped = new Result(true, false, "GitHub limited update checks (60 per hour per internet connection), so this check was skipped. "
                + "Started the installed version " + installedVersion + ". Checks work again " + when + ".", "", "", checked);
            skipped.limited = true;
            return skipped;
        } catch (Exception e) {
            String reason = e instanceof SocketTimeoutException ? "GitHub took too long to respond." :
                e instanceof UnknownHostException ? "GitHub could not be reached. Check your internet connection." :
                e.getMessage() == null ? "GitHub could not be reached." : shortText(e.getMessage(), 180);
            return new Result(true, false, reason + "\n\nRetry the check or start the installed version.", "", "", checked);
        }
    }

    static String newerAndroidRelease(JSONObject release, String installed) {
        if (release == null || release.optBoolean("draft") || release.optBoolean("prerelease")) return "";
        // Android releases use android-v<major>.<minor>[.<patch>], with the exact APK asset name.
        Matcher tag = Pattern.compile("^android-v(\\d+\\.\\d+(?:\\.\\d+)?)$").matcher(release.optString("tag_name"));
        if (!tag.matches() || compareVersions(tag.group(1), installed) <= 0) return "";
        return verifiedAsset(release).isEmpty()?"":tag.group(1);
    }

    static String verifiedAsset(JSONObject release) {
        if(release==null)return "";
        String tag=release.optString("tag_name");
        if(!tag.matches("android-v\\d+\\.\\d+(?:\\.\\d+)?")||!release.optString("html_url").equals(REPOSITORY+"/releases/tag/"+tag))return "";
        JSONArray assets = release.optJSONArray("assets");
        if (assets != null) for (int i = 0; i < assets.length(); i++) {
            JSONObject asset = assets.optJSONObject(i);
            String expected=REPOSITORY+"/releases/download/"+tag+"/GoHost.apk";
            if (asset != null && "GoHost.apk".equals(asset.optString("name")) && "uploaded".equals(asset.optString("state")) && asset.optLong("size")>0 && expected.equals(asset.optString("browser_download_url"))) return expected;
        }
        return "";
    }

    /** Accepts `sha256sum` output ("<hex>  GoHost.apk") or a bare hex digest. */
    static String parseChecksum(String text) throws IOException {
        Matcher m = Pattern.compile("^\\s*([0-9a-fA-F]{64})(?:\\s+\\*?GoHost\\.apk)?\\s*$").matcher(text == null ? "" : text);
        if (!m.matches()) throw new IOException("The release checksum file is not valid.");
        return m.group(1).toLowerCase(java.util.Locale.ROOT);
    }

    /** The published SHA-256 file for the release APK, or "" when it is missing. */
    static String checksumAsset(JSONObject release) {
        if (verifiedAsset(release).isEmpty()) return "";
        String tag = release.optString("tag_name");
        String expected = REPOSITORY + "/releases/download/" + tag + "/GoHost.apk.sha256";
        JSONArray assets = release.optJSONArray("assets");
        if (assets != null) for (int i = 0; i < assets.length(); i++) {
            JSONObject asset = assets.optJSONObject(i);
            if (asset != null && "GoHost.apk.sha256".equals(asset.optString("name")) && "uploaded".equals(asset.optString("state"))
                && asset.optLong("size") > 0 && asset.optLong("size") < 1024 && expected.equals(asset.optString("browser_download_url"))) return expected;
        }
        return "";
    }

    static int compareVersions(String left, String right) {
        try {
            String[] a = left.split("\\."), b = right.split("\\.");
            if (!left.matches("\\d+\\.\\d+(?:\\.\\d+)?") || !right.matches("\\d+\\.\\d+(?:\\.\\d+)?")) return 0;
            for (int i = 0; i < Math.max(a.length, b.length); i++) {
                int comparison = Long.compare(i < a.length ? Long.parseLong(a[i]) : 0, i < b.length ? Long.parseLong(b[i]) : 0);
                if (comparison != 0) return comparison;
            }
        } catch (NumberFormatException ignored) { }
        return 0;
    }

    private static String shortText(String value, int limit) {
        value = value.trim();
        return value.length() <= limit ? value : value.substring(0, limit) + "…";
    }

    static JSONObject parse(String path, String body) throws Exception {
        if (path.equals(LATEST_COMMIT)) {
            JSONArray commits = new JSONArray(body);
            if (commits.length() == 0) throw new IOException("GitHub returned no published commits.");
            return commits.getJSONObject(0);
        }
        return new JSONObject(body);
    }

    /**
     * Conditional requests: each answer is stored with its ETag and revalidated
     * with If-None-Match. GitHub does not count 304 replies against the 60/hour
     * anonymous limit. When GitHub refuses, the last known answer is used.
     */
    private static final class Network implements Fetcher {
        private final File cacheDir;
        Network(File cacheDir) { this.cacheDir = cacheDir; }

        private File cacheFile(String path) {
            return cacheDir == null ? null : new File(cacheDir, "github-" + Integer.toHexString(path.hashCode()) + ".json");
        }
        private JSONObject cached(String path) {
            File f = cacheFile(path);
            try { return f != null && f.isFile() ? new JSONObject(new String(java.nio.file.Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8)) : null; }
            catch (Exception e) { return null; }
        }
        private void store(String path, String etag, String body, boolean missing) {
            File f = cacheFile(path);
            if (f == null) return;
            try {
                JSONObject entry = new JSONObject().put("etag", etag == null ? "" : etag).put("missing", missing).put("body", body == null ? "" : body);
                java.nio.file.Files.write(f.toPath(), entry.toString().getBytes(StandardCharsets.UTF_8));
            } catch (Exception ignored) { }
        }
        private JSONObject fromCache(String path, JSONObject entry, boolean allowMissing) throws Exception {
            if (entry.optBoolean("missing")) { if (allowMissing) return null; throw new IOException("GitHub returned no data."); }
            return parse(path, entry.getString("body"));
        }

        public JSONObject get(String path, boolean allowMissing) throws Exception {
            if (Thread.currentThread().isInterrupted()) throw new InterruptedIOException("Check canceled");
            JSONObject entry = cached(path);
            HttpURLConnection connection = (HttpURLConnection)new URL(API + path).openConnection();
            try {
                connection.setConnectTimeout(4000); connection.setReadTimeout(4000);
                connection.setUseCaches(false);
                connection.setRequestProperty("Accept", "application/vnd.github+json");
                connection.setRequestProperty("User-Agent", "DominoGo-Host/" + BuildConfig.VERSION_NAME);
                if (entry != null && !entry.optString("etag").isEmpty()) connection.setRequestProperty("If-None-Match", entry.optString("etag"));
                int code = connection.getResponseCode();
                if (code == 304 && entry != null) return fromCache(path, entry, allowMissing);
                if (code == 404 && allowMissing) { store(path, connection.getHeaderField("ETag"), "", true); return null; }
                if (code == 403 || code == 429) {
                    if (entry != null) return fromCache(path, entry, allowMissing);
                    long reset = connection.getHeaderFieldLong("X-RateLimit-Reset", 0) * 1000L;
                    throw new RateLimited(reset);
                }
                if (code != 200) throw new IOException("GitHub update check failed (HTTP " + code + ").");
                try (InputStream in = connection.getInputStream(); ByteArrayOutputStream out = new ByteArrayOutputStream()) {
                    byte[] bytes = new byte[8192]; int n;
                    while ((n = in.read(bytes)) > 0) {
                        if (Thread.currentThread().isInterrupted()) throw new InterruptedIOException("Check canceled");
                        if (out.size() + n > 1_048_576) throw new IOException("GitHub response was too large.");
                        out.write(bytes, 0, n);
                    }
                    String body = new String(out.toByteArray(), StandardCharsets.UTF_8);
                    JSONObject parsed = parse(path, body);
                    store(path, connection.getHeaderField("ETag"), body, false);
                    return parsed;
                }
            } finally { connection.disconnect(); }
        }
    }
}
