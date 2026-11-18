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
    private JSONObject withChecksum(JSONObject release,String url,long size)throws Exception{release.getJSONArray("assets").put(new JSONObject().put("name","GoHost.apk.sha256").put("state","uploaded").put("size",size).put("browser_download_url",url));return release;}
    @Test public void selfInstallNeedsPublishedChecksumNextToApk()throws Exception{
        String tag="android-v2.3.0",base=GitHubUpdates.REPOSITORY+"/releases/download/"+tag+"/",apk=base+"GoHost.apk";
        assertEquals("",GitHubUpdates.checksumAsset(release(tag,"GoHost.apk",apk)));
        assertEquals(base+"GoHost.apk.sha256",GitHubUpdates.checksumAsset(withChecksum(release(tag,"GoHost.apk",apk),base+"GoHost.apk.sha256",80)));
        assertEquals("",GitHubUpdates.checksumAsset(withChecksum(release(tag,"GoHost.apk",apk),"https://example.com/GoHost.apk.sha256",80)));
        assertEquals("",GitHubUpdates.checksumAsset(withChecksum(release(tag,"GoHost.apk",apk),base+"GoHost.apk.sha256",5000)));
        JSONObject full=withChecksum(release(tag,"GoHost.apk",apk),base+"GoHost.apk.sha256",80);
        GitHubUpdates.Result r=new GitHubUpdates((p,m)->p.equals(GitHubUpdates.LATEST_COMMIT)?commit():full,"","2.2.3").check();
        assertTrue(r.installable());assertEquals("2.3.0",r.version);assertEquals(apk,r.url);
        JSONObject noSum=release(tag,"GoHost.apk",apk);
        assertFalse(new GitHubUpdates((p,m)->p.equals(GitHubUpdates.LATEST_COMMIT)?commit():noSum,"","2.2.3").check().installable());
    }
    @Test public void rateLimitSkipsCheckInsteadOfBlockingStart()throws Exception{
        GitHubUpdates.Result r=new GitHubUpdates((p,m)->{throw new GitHubUpdates.RateLimited(0);},revision,"2.3.0").check();
        assertTrue(r.limited);assertTrue(r.failed);assertFalse(r.installable());
        assertTrue(r.message.contains("Started the installed version 2.3.0"));
        GitHubUpdates.Result offline=new GitHubUpdates((p,m)->{throw new IOException("Offline");},revision,"2.3.0").check();
        assertFalse(offline.limited);assertTrue(offline.failed);
    }
    @Test public void checksumFileFormats()throws Exception{
        String hex="85d349b1130b3be447566dd09e19f16e0587f0a047cfa793e515e72169da56bf";
        assertEquals(hex,GitHubUpdates.parseChecksum(hex));
        assertEquals(hex,GitHubUpdates.parseChecksum(hex.toUpperCase()+"  GoHost.apk\n"));
        assertEquals(hex,GitHubUpdates.parseChecksum(hex+" *GoHost.apk"));
        for(String bad:new String[]{"",hex.substring(1),hex+"  other.apk","not a checksum"}){
            try{GitHubUpdates.parseChecksum(bad);fail(bad);}catch(IOException expected){}
        }
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
