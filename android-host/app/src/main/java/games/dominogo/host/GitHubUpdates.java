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

    static final class Result {
        final boolean failed, news;
        final String message, url, revision;
        final long checked;
        Result(boolean failed, boolean news, String message, String url, String revision, long checked) {
            this.failed = failed; this.news = news; this.message = message;
            this.url = url; this.revision = revision; this.checked = checked;
        }
        JSONObject json() throws Exception {
            return new JSONObject().put("failed", failed).put("news", news).put("message", message)
                .put("url", url).put("revision", revision).put("checked", checked);
        }
    }

    private final Fetcher fetcher;
    private final String installedRevision, installedVersion;
    GitHubUpdates() { this(new Network(), BuildConfig.SOURCE_REVISION, BuildConfig.VERSION_NAME); }
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
                return new Result(false, true, "Android update " + androidRelease + " is available. Installed: " + installedVersion + "."
                    + (notes.isEmpty() ? "" : "\n\n" + notes), url, sha, checked);
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

    private static final class Network implements Fetcher {
        public JSONObject get(String path, boolean allowMissing) throws Exception {
            if (Thread.currentThread().isInterrupted()) throw new InterruptedIOException("Check canceled");
            HttpURLConnection connection = (HttpURLConnection)new URL(API + path).openConnection();
            try {
                connection.setConnectTimeout(4000); connection.setReadTimeout(4000);
                connection.setUseCaches(false);
                connection.setRequestProperty("Cache-Control", "no-cache");
                connection.setRequestProperty("Accept", "application/vnd.github+json");
                connection.setRequestProperty("User-Agent", "DominoGo-Host/" + BuildConfig.VERSION_NAME);
                int code = connection.getResponseCode();
                if (code == 404 && allowMissing) return null;
                if (code == 403 || code == 429) throw new IOException("GitHub limited update checks. Try again later.");
                if (code != 200) throw new IOException("GitHub update check failed (HTTP " + code + ").");
                try (InputStream in = connection.getInputStream(); ByteArrayOutputStream out = new ByteArrayOutputStream()) {
                    byte[] bytes = new byte[8192]; int n;
                    while ((n = in.read(bytes)) > 0) {
                        if (Thread.currentThread().isInterrupted()) throw new InterruptedIOException("Check canceled");
                        if (out.size() + n > 1_048_576) throw new IOException("GitHub response was too large.");
                        out.write(bytes, 0, n);
                    }
                    String body = new String(out.toByteArray(), StandardCharsets.UTF_8);
                    if (path.equals(LATEST_COMMIT)) {
                        JSONArray commits = new JSONArray(body);
                        if (commits.length() == 0) throw new IOException("GitHub returned no published commits.");
                        return commits.getJSONObject(0);
                    }
                    return new JSONObject(body);
                }
            } finally { connection.disconnect(); }
        }
    }
}
