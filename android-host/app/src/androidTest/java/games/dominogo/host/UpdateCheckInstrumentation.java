package games.dominogo.host;

import android.app.Instrumentation;
import android.os.Bundle;
import org.json.*;
import java.io.IOException;
import java.util.*;

/** On-device contract tests: Android's real JSON implementation, no network fixtures in the APK. */
public class UpdateCheckInstrumentation extends TouchGameInstrumentation {
    private boolean touchMode;
    private static final String INSTALLED = "1111111111111111111111111111111111111111";
    private static final String NEW = "2222222222222222222222222222222222222222";
    private int passed;
    public void onCreate(Bundle arguments) { touchMode=arguments.getString("url")!=null;super.onCreate(arguments); }
    public void onStart() {
        if(touchMode){super.onStart();return;}
        Bundle results = new Bundle();
        try {
            tests(); results.putString("stream", "\n" + passed + " GitHub update checks passed\n"); finish(-1, results);
        } catch (Throwable e) { results.putString("stream", "\nFAILED: " + e + "\n"); finish(0, results); }
    }
    private void expect(boolean value, String detail) { if (!value) throw new AssertionError(detail); passed++; }
    private JSONObject commit(String sha) throws Exception {
        return new JSONObject().put("sha", sha).put("commit", new JSONObject().put("message", "Improve game\nMore details"));
    }
    private JSONObject release(String tag, boolean apk) throws Exception {
        JSONArray assets = new JSONArray();
        if (apk) assets.put(new JSONObject().put("name", "GoHost.apk").put("state", "uploaded").put("size",12345).put("browser_download_url",GitHubUpdates.REPOSITORY+"/releases/download/"+tag+"/GoHost.apk"));
        return new JSONObject().put("tag_name", tag).put("html_url", GitHubUpdates.REPOSITORY + "/releases/tag/" + tag).put("assets", assets).put("body", "New game features");
    }
    private GitHubUpdates.Result check(String sha, JSONObject release, String compare) throws Exception {
        JSONObject latest = commit(sha);
        return new GitHubUpdates((path, missing) -> path.equals(GitHubUpdates.LATEST_COMMIT) ? latest : path.equals("/releases/latest") ? release :
            compare == null ? null : new JSONObject().put("status", compare).put("ahead_by", 3), INSTALLED, "1.1").check();
    }
    private void tests() throws Exception {
        GitHubUpdates.Result same = check(INSTALLED, null, null);
        expect(!same.failed && !same.news, "A repository without releases and no new commit should allow Start");
        GitHubUpdates.Result ahead = check(NEW, null, "ahead");
        expect(ahead.news && !ahead.failed && ahead.message.contains("3 new") && ahead.message.contains("rebuilt APK"), "New source changes must be distinguished from an installable APK");
        expect(ahead.url.contains("/compare/" + INSTALLED + "..." + NEW), "Review points to the actual source comparison");
        expect(check(NEW, null, "behind").news, "A remote branch behind the installed build must be reported honestly");
        expect(check(NEW, null, "diverged").news, "Diverged source branches must be reported");
        expect(check(NEW, null, null).news, "An unavailable comparison must not hide a changed repository");
        GitHubUpdates.Result apk = check(INSTALLED, release("android-v1.2", true), null);
        expect(apk.news && apk.message.contains("Android update 1.2"), "A newer stable Android APK release is offered");
        expect(!check(INSTALLED, release("android-v1.2", false), null).news, "A tag without the Android APK is not installable");
        expect(!check(INSTALLED, release("android-v1.0", true), null).news, "Never offer an older APK");
        expect(!check(INSTALLED, release("android-v1.1.0", true), null).news, "Equivalent version numbers must not produce an update loop");
        JSONObject preview = release("android-v1.2", true).put("prerelease", true);
        expect(!check(INSTALLED, preview, null).news, "Do not offer preview releases as stable updates");
        expect(!check(INSTALLED, release("v5.0", true), null).news, "Unrelated release tags must not be treated as Android updates");
        expect(GitHubUpdates.compareVersions("1.10", "1.9") > 0, "Compare version numbers numerically");
        GitHubUpdates.Result unavailable = new GitHubUpdates((p, m) -> { throw new IOException("GitHub limited update checks."); }, INSTALLED, "1.1").check();
        expect(unavailable.failed && !unavailable.news && unavailable.message.contains("start the installed"), "Failure offers an explicit installed-version fallback instead of pretending to be current");
        expect(new GitHubUpdates((p, m) -> commit("bad"), INSTALLED, "1.1").check().failed, "Reject invalid revision metadata");
        final int[] requests = {0};
        GitHubUpdates checker = new GitHubUpdates((p, m) -> { requests[0]++; return p.equals(GitHubUpdates.LATEST_COMMIT) ? commit(INSTALLED) : null; }, INSTALLED, "1.1");
        checker.check(); checker.check();
        expect(requests[0] == 4, "Every Start check must make fresh requests for commits and releases");
        GitHubUpdates.Result unknown = new GitHubUpdates((p,m) -> p.equals(GitHubUpdates.LATEST_COMMIT) ? commit(INSTALLED) : null, "", "1.1").check();
        expect(!unknown.failed && !unknown.news && unknown.url.isEmpty() && unknown.message.contains("locally built version"), "Local build metadata must not block Start after a successful app-update check");
        expect(!unknown.message.contains("No newer published changes"), "An uncomparable local build cannot claim its source is current");
        GitHubUpdates.Result localUpdate = new GitHubUpdates((p,m) -> p.equals(GitHubUpdates.LATEST_COMMIT) ? commit(INSTALLED) : release("android-v1.2", true), "", "1.1").check();
        expect(!localUpdate.failed && localUpdate.news && localUpdate.url.endsWith("/GoHost.apk"), "A local build still offers a verified newer Android app");
    }
}
