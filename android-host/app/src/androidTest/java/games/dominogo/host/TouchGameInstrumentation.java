package games.dominogo.host;

import android.app.*;
import android.content.*;
import android.os.*;
import android.view.*;
import android.webkit.*;
import android.widget.FrameLayout;
import org.json.*;
import java.util.concurrent.*;

/** Test APK only: real Android WebView rendering and injected OS touch events.
 * The coordinator supplies an isolated server over ADB reverse, never player data.
 */
public class TouchGameInstrumentation extends Instrumentation {
    private String url; private Activity activity; private WebView web;
    public void onCreate(Bundle args){super.onCreate(args);url=args.getString("url");start();}
    private String js(String source)throws Exception{
        CountDownLatch latch=new CountDownLatch(1);String[] answer={"null"};
        runOnMainSync(()->web.evaluateJavascript(source,value->{answer[0]=value;latch.countDown();}));
        if(!latch.await(5,TimeUnit.SECONDS))throw new AssertionError("WebView query timed out");
        return answer[0];
    }
    private void waitFor(String condition)throws Exception{
        for(int i=0;i<150;i++){if("true".equals(js(condition)))return;Thread.sleep(200);}
        throw new AssertionError("Timed out: "+condition+"; "+js("JSON.stringify({heading:document.querySelector('.board-status h1')?.textContent,selected:[...document.querySelectorAll('.hand-card.selected')].map(e=>e.getAttribute('aria-label')),cards:document.querySelectorAll('.hand-card').length})"));
    }
    private JSONObject object(String expression)throws Exception{
        return new JSONObject((String)new JSONTokener(js("JSON.stringify("+expression+")")).nextValue());
    }
    private void touch(long down,int action,float x,float y){
        MotionEvent e=MotionEvent.obtain(down,SystemClock.uptimeMillis(),action,x,y,0);
        e.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
        if(!getUiAutomation().injectInputEvent(e,true))throw new AssertionError("Touch injection failed");
        e.recycle();
    }
    private void stage(String name){Bundle b=new Bundle();b.putString("stream","TOUCH_STAGE="+name+"\n");sendStatus(1,b);}
    private void tapSelector(String selector)throws Exception{
        JSONObject point=object("(()=>{const e=document.querySelector("+JSONObject.quote(selector)+");const r=e.getBoundingClientRect();let point=null,best=Infinity;for(let y=r.top+18;y<r.bottom-12;y+=5)for(let x=r.left+8;x<r.right-8;x+=5)if(e.contains(document.elementFromPoint(x,y))){const score=(x-r.x-r.width/2)**2+.25*(y-r.y-r.height/2)**2;if(score<best){best=score;point={x,y,width:innerWidth};}}if(point)return point;throw Error('No exposed touch target');})()");
        int[] location=new int[2],width=new int[1];runOnMainSync(()->{web.getLocationOnScreen(location);width[0]=web.getWidth();});
        float scale=(float)(width[0]/point.getDouble("width"));
        float x=location[0]+(float)point.getDouble("x")*scale,y=location[1]+(float)point.getDouble("y")*scale;
        long down=SystemClock.uptimeMillis();touch(down,MotionEvent.ACTION_DOWN,x,y);Thread.sleep(60);touch(down,MotionEvent.ACTION_UP,x,y);
    }
    public void onStart(){
        Bundle result=new Bundle();boolean ok=false;
        try{
            if(url==null||!url.startsWith("http://127.0.0.1:3311/?join="))throw new AssertionError("Isolated test URL required");
            activity=startActivitySync(new Intent(getTargetContext(),MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            runOnMainSync(()->{
                web=new WebView(activity);web.getSettings().setJavaScriptEnabled(true);web.getSettings().setDomStorageEnabled(true);
                web.setWebViewClient(new WebViewClient());
                FrameLayout.LayoutParams layout=new FrameLayout.LayoutParams(-1,-1);
                WindowInsets insets=activity.getWindow().getDecorView().getRootWindowInsets();
                if(insets!=null&&Build.VERSION.SDK_INT>=30){android.graphics.Insets safe=insets.getInsets(WindowInsets.Type.systemBars()|WindowInsets.Type.displayCutout());layout.setMargins(safe.left,safe.top,safe.right,safe.bottom);}
                activity.addContentView(web,layout);web.loadUrl(url);
            });
            stage("waiting-for-isolated-room");
            waitFor("document.querySelectorAll('.hand-card').length===7");Thread.sleep(900);
            JSONObject geometry=object("(()=>{const e=document.querySelector('.hand-card.playable');if(!e)return {};const r=e.getBoundingClientRect();let p=null;for(let y=r.top+18;y<r.bottom-10&&!p;y+=6)for(let x=r.left+8;x<r.right-8;x+=6)if(document.elementFromPoint(x,y)?.closest('[data-card-id]')===e){p={x,y};break;}const d=document.querySelector('.discard-pile').getBoundingClientRect();return {source:p,target:{x:d.x+d.width/2,y:d.y+d.height/2},width:innerWidth,id:e.dataset.cardId};})()") ;
            int[] location=new int[2];int[] width=new int[1];runOnMainSync(()->{web.getLocationOnScreen(location);width[0]=web.getWidth();});
            float scale=(float)(width[0]/geometry.getDouble("width"));
            JSONObject s=geometry.getJSONObject("source"),t=geometry.getJSONObject("target");
            float x=location[0]+(float)s.getDouble("x")*scale,y=location[1]+(float)s.getDouble("y")*scale;
            float tx=location[0]+(float)t.getDouble("x")*scale,ty=location[1]+(float)t.getDouble("y")*scale;
            stage("before-drag");Thread.sleep(1300);
            long down=SystemClock.uptimeMillis();touch(down,MotionEvent.ACTION_DOWN,x,y);Thread.sleep(220);
            for(int i=1;i<=14;i++){touch(down,MotionEvent.ACTION_MOVE,x+(tx-x)*i/14,y+(ty-y)*i/14);Thread.sleep(18);}
            waitFor("!!document.querySelector('.game-board.drop-ready')&&!!document.querySelector('.card-flight.is-held')");
            stage("held-over-pile");Thread.sleep(1300);touch(down,MotionEvent.ACTION_UP,tx,ty);
            waitFor("document.querySelectorAll('.hand-card').length===6");
            waitFor("!document.querySelector('.card-flight')");
            if(!"true".equals(js("document.documentElement.scrollHeight<=innerHeight+1&&document.documentElement.scrollWidth<=innerWidth")))throw new AssertionError("Gameplay overflowed the physical screen");
            stage("after-drag");Thread.sleep(1300);
            waitFor("!!document.querySelector('.touch-target.ready')");
            JSONObject tap=object("(()=>{const r=document.querySelector('.touch-target').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()");
            float tapX=location[0]+(float)tap.getDouble("x")*scale,tapY=location[1]+(float)tap.getDouble("y")*scale;
            down=SystemClock.uptimeMillis();touch(down,MotionEvent.ACTION_DOWN,tapX,tapY);Thread.sleep(60);touch(down,MotionEvent.ACTION_UP,tapX,tapY);
            waitFor("!!document.querySelector('.touch-target.tapped')&&!!document.querySelector('.touch-stamp')");
            stage("five-tap-ack");Thread.sleep(1000);
            waitFor("!!document.querySelector('.tap-outcome')");
            stage("five-result");Thread.sleep(1000);
            JSONObject emote=object("(()=>{const r=document.querySelector('.reaction-button').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()");
            float ex=location[0]+(float)emote.getDouble("x")*scale,ey=location[1]+(float)emote.getDouble("y")*scale;
            down=SystemClock.uptimeMillis();touch(down,MotionEvent.ACTION_DOWN,ex,ey);Thread.sleep(60);touch(down,MotionEvent.ACTION_UP,ex,ey);
            waitFor("!!document.querySelector('.own-reaction')&&getComputedStyle(document.querySelector('.own-reaction')).visibility==='visible'");
            waitFor("!!document.querySelector('.reaction-bubble:not(.own-reaction)')");
            stage("own-and-opponent-emotes");Thread.sleep(1000);
            waitFor("document.querySelector('.board-status h1')?.textContent==='Your turn'");
            tapSelector(".hand-card[aria-label^='Red 8']");
            waitFor("document.querySelectorAll('.hand-card.selected').length===1");
            tapSelector(".hand-card[aria-label^='Blue 8']");
            waitFor("document.querySelectorAll('.card-order-badge').length===2&&!!document.querySelector('.play-group:not(:disabled)')");
            if(!"true".equals(js("!document.querySelector('.group-toggle')")))throw new AssertionError("Obsolete Group button remains");
            stage("direct-number-selection");Thread.sleep(1000);
            tapSelector(".play-group");waitFor("document.querySelectorAll('.hand-card').length===4");
            stage("after-number-group");Thread.sleep(1000);
            result.putString("stream","PASS: physical Android touch drag, 5 tap, own/remote emotes and direct number grouping\n");ok=true;
        }catch(Throwable e){result.putString("stream","FAIL: "+e.toString()+"\n");}
        finally{
            if(web!=null)runOnMainSync(()->{((android.view.ViewGroup)web.getParent()).removeView(web);web.destroy();WebStorage.getInstance().deleteOrigin("http://127.0.0.1:3311");});
            finish(ok?-1:0,result);
        }
    }
}
