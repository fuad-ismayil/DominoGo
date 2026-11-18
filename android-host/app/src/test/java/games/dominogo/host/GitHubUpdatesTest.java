package games.dominogo.host;
import org.junit.Test;
import org.json.*;
import java.io.IOException;
import static org.junit.Assert.*;

public class GitHubUpdatesTest {
    private final String revision="1111111111111111111111111111111111111111";
    private JSONObject commit()throws Exception{return new JSONObject().put("sha",revision).put("commit",new JSONObject().put("message","Shared host"));}
    private JSONObject release(String tag,String asset,String url)throws Exception{return new JSONObject().put("tag_name",tag).put("html_url",GitHubUpdates.REPOSITORY+"/releases/tag/"+tag).put("assets",new JSONArray().put(new JSONObject().put("name",asset).put("state","uploaded").put("size",123).put("browser_download_url",url)));}
    @Test public void sharedApkNeedsVerifiedRepositoryAsset()throws Exception{
        String tag="android-v2.1.0",url=GitHubUpdates.REPOSITORY+"/releases/download/"+tag+"/GoHost.apk";
        assertEquals("2.1.0",GitHubUpdates.newerAndroidRelease(release(tag,"GoHost.apk",url),"2.0.0"));
        assertEquals("",GitHubUpdates.newerAndroidRelease(release(tag,"DominoGo-Host.apk",url),"2.0.0"));
        assertEquals("",GitHubUpdates.newerAndroidRelease(release(tag,"GoHost.apk","https://example.com/GoHost.apk"),"2.0.0"));
        assertEquals("",GitHubUpdates.newerAndroidRelease(release(tag,"GoHost.apk",url),"2.2.0"));
        assertEquals("",GitHubUpdates.newerAndroidRelease(release(tag,"GoHost.apk",url).put("prerelease",true),"2.0.0"));
    }
    @Test public void dirtyBuildCannotClaimPublishedRevision()throws Exception{
        GitHubUpdates.Result r=new GitHubUpdates((p,m)->p.equals(GitHubUpdates.LATEST_COMMIT)?commit():null,"","2.0.0").check();
        assertFalse(r.failed);assertFalse(r.news);assertEquals("",r.url);
        assertTrue(r.message.contains("locally built version"));
        assertFalse(r.message.contains("No newer published changes"));
    }
    @Test public void localBuildStillOffersVerifiedNewerApk()throws Exception{
        String tag="android-v2.2.0",url=GitHubUpdates.REPOSITORY+"/releases/download/"+tag+"/GoHost.apk";
        JSONObject update=release(tag,"GoHost.apk",url);
        GitHubUpdates.Result r=new GitHubUpdates((p,m)->p.equals(GitHubUpdates.LATEST_COMMIT)?commit():update,"","2.1.1").check();
        assertFalse(r.failed);assertTrue(r.news);assertEquals(url,r.url);
    }
    @Test public void eachStartMakesFreshRequestsAndOfflineHasFallback()throws Exception{
        int[] requests={0};GitHubUpdates checker=new GitHubUpdates((p,m)->{requests[0]++;return p.equals(GitHubUpdates.LATEST_COMMIT)?commit():null;},revision,"2.0.0");
        assertFalse(checker.check().failed);assertFalse(checker.check().news);assertEquals(4,requests[0]);
        assertTrue(new GitHubUpdates((p,m)->{throw new IOException("Offline");},revision,"2.0.0").check().failed);
        assertTrue(GitHubUpdates.compareVersions("2.10","2.9")>0);
    }
    @Test public void behindAndDivergedAreReported()throws Exception{
        for(String status:new String[]{"behind","diverged"}){
            GitHubUpdates.Result r=new GitHubUpdates((p,m)->p.equals(GitHubUpdates.LATEST_COMMIT)?commit().put("sha","2222222222222222222222222222222222222222"):p.equals("/releases/latest")?null:new JSONObject().put("status",status),revision,"2.0.0").check();
            assertTrue(r.news);assertTrue(r.url.contains("/compare/"));
        }
    }
}
