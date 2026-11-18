package games.dominogo.host;

import android.Manifest;
import android.app.*;
import android.content.*;
import android.content.res.Configuration;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.*;
import android.text.InputType;
import android.view.*;
import android.widget.*;
import org.json.JSONObject;
import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;

/** A GitHub check or one game's Stop never stops the other game. */
public class MainActivity extends Activity {
    private final Handler handler=new Handler(Looper.getMainLooper());
    private final Map<String,Row> rows=new LinkedHashMap<>();
    private TextView tunnel,sync,updates; private Button both,stopAll,settings,review;
    private int surface,text,secondary,primary,onPrimary,generation;
    private boolean checking,resumed,active; private Thread checkThread;
    private String target="both"; private GitHubUpdates.Result pending;
    private GitHubUpdates.Result pendingUpdate; private String pendingUpdateGame="both"; private Thread updateThread;
    private static class Row {String id,url="",local="",phase="stopped";TextView state,detail,link;Button start,stop,open,share,copy;}
    private final Runnable poll=new Runnable(){public void run(){refresh();handler.postDelayed(this,1000);}};

    public void onCreate(Bundle saved){super.onCreate(saved);
        boolean dark=(getResources().getConfiguration().uiMode&Configuration.UI_MODE_NIGHT_MASK)==Configuration.UI_MODE_NIGHT_YES;
        surface=Color.parseColor(dark?"#091A30":"#F1F5FA");text=Color.parseColor(dark?"#F1F7FF":"#142E4C");secondary=Color.parseColor(dark?"#C0D3EA":"#425D79");primary=Color.parseColor(dark?"#98D0FF":"#225E98");onPrimary=Color.parseColor(dark?"#102B48":"#FFFFFF");
        getWindow().setStatusBarColor(surface);getWindow().setNavigationBarColor(surface);getWindow().getDecorView().setSystemUiVisibility(dark?0:View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR|View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
        ScrollView scroll=new ScrollView(this);scroll.setFillViewport(true);scroll.setBackgroundColor(surface);LinearLayout root=column();scroll.addView(root);
        scroll.setOnApplyWindowInsetsListener((v,i)->{root.setPadding(dp(24)+i.getSystemWindowInsetLeft(),dp(20)+i.getSystemWindowInsetTop(),dp(24)+i.getSystemWindowInsetRight(),dp(24)+i.getSystemWindowInsetBottom());return i;});
        TextView title=label("Go Host",28,text);title.setTypeface(null,Typeface.BOLD);add(root,title,0);add(root,label("Host either game, or both. Your existing accounts stay with you.",16,secondary),8);
        both=button("Start both",true);stopAll=button("Stop all",false);add(root,pair(both,stopAll),24);both.setOnClickListener(v->check("both"));stopAll.setOnClickListener(v->{cancelCheck();stop("both");});
        gameRow(root,"dominogo","DominoGo",3000);gameRow(root,"unogo","UnoGo",3001);
        tunnel=label("Public links appear after Start. Stopping one game preserves the other.",14,secondary);add(root,tunnel,24);
        add(root,label("GitHub updates",20,text),28);updates=label("Checks before every Start.",14,secondary);add(root,updates,8);
        review=button("View update on GitHub",false);add(root,review,12);review.setOnClickListener(v->{String url=read("update-state.json").optString("url");if(url.startsWith(GitHubUpdates.REPOSITORY+"/"))open(url);});
        sync=label("PC sync waits for both games to stop on both hosts. Connect the paired phone by USB or wireless debugging.",14,secondary);add(root,sync,24);
        settings=button("Public link settings",false);add(root,settings,20);settings.setOnClickListener(v->settings());Button logs=button("Connection details",false);add(root,logs,8);logs.setOnClickListener(v->logs());
        add(root,label("Keep the phone online while hosting. When a newer Go Host is published on GitHub, Start downloads it, verifies it, saves and stops the games, installs it and starts them again.",14,secondary),20);
        setContentView(scroll);scroll.requestApplyInsets();
    }
    private void gameRow(LinearLayout root,String id,String name,int port){
        View line=new View(this);line.setBackgroundColor(secondary);LinearLayout.LayoutParams rule=new LinearLayout.LayoutParams(-1,dp(1));rule.topMargin=dp(26);root.addView(line,rule);
        Row r=new Row();r.id=id;r.local="http://127.0.0.1:"+port;rows.put(id,r);TextView title=label(name,23,text);title.setTypeface(null,Typeface.BOLD);add(root,title,22);
        r.state=label("Stopped",18,text);r.detail=label("Start this game independently.",14,secondary);add(root,r.state,8);add(root,r.detail,6);
        r.start=button("Start",true);r.stop=button("Stop",false);add(root,pair(r.start,r.stop),12);r.start.setOnClickListener(v->check(id));r.stop.setOnClickListener(v->{if(checking&&(target.equals(id)||target.equals("both")))cancelCheck();stop(id);});
        r.link=label("Public link not verified yet.",14,secondary);r.link.setTextIsSelectable(true);add(root,r.link,12);
        r.open=button("Open game",false);r.share=button("Share link",false);r.copy=button("Copy link",false);LinearLayout links=new LinearLayout(this);
        for(Button b:new Button[]{r.open,r.share,r.copy}){LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-2,1);lp.setMarginEnd(dp(6));links.addView(b,lp);}add(root,links,8);
        r.open.setOnClickListener(v->open(r.local));r.share.setOnClickListener(v->startActivity(Intent.createChooser(new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT,r.url),"Invite players to "+name)));
        r.copy.setOnClickListener(v->{getSystemService(ClipboardManager.class).setPrimaryClip(ClipData.newPlainText(name,r.url));r.copy.setText("Copied");handler.postDelayed(()->r.copy.setText("Copy link"),1600);});
    }
    private LinearLayout column(){LinearLayout v=new LinearLayout(this);v.setOrientation(LinearLayout.VERTICAL);return v;}
    private int dp(int n){return Math.round(n*getResources().getDisplayMetrics().density);}
    private TextView label(String value,int size,int color){TextView v=new TextView(this);v.setText(value);v.setTextSize(size);v.setTextColor(color);v.setLineSpacing(dp(2),1);return v;}
    private void add(LinearLayout root,View view,int gap){LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.topMargin=dp(gap);root.addView(view,lp);}
    private LinearLayout pair(Button a,Button b){LinearLayout v=new LinearLayout(this);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-2,1);lp.setMarginEnd(dp(8));v.addView(a,lp);v.addView(b,new LinearLayout.LayoutParams(0,-2,1));return v;}
    private Button button(String title,boolean filled){Button b=new Button(this);b.setText(title);b.setTextSize(15);b.setAllCaps(false);b.setMinHeight(dp(52));b.setPadding(dp(10),dp(10),dp(10),dp(10));b.setTextColor(filled?onPrimary:text);GradientDrawable bg=new GradientDrawable();bg.setCornerRadius(dp(14));bg.setColor(filled?primary:Color.TRANSPARENT);if(!filled)bg.setStroke(dp(1),secondary);b.setBackground(new android.graphics.drawable.RippleDrawable(android.content.res.ColorStateList.valueOf(0x3398D0FF),bg,null));return b;}
    private JSONObject read(String name){try{return new JSONObject(new String(Files.readAllBytes(new File(getFilesDir(),name).toPath()),StandardCharsets.UTF_8));}catch(Exception e){return new JSONObject();}}
    private void refresh(){
        JSONObject status=read("host-state.json"),games=status.optJSONObject("games");boolean fresh=System.currentTimeMillis()-status.optLong("updated")<12000;active=false;
        for(Row r:rows.values()){
            JSONObject g=games==null?new JSONObject():games.optJSONObject(r.id);if(g==null)g=new JSONObject();String phase=fresh?g.optString("phase","stopped"):"stopped";r.phase=phase;
            boolean enabled=fresh&&g.optBoolean("enabled"),busy=phase.equals("starting")||phase.equals("stopping"),running=enabled||busy;active|=running;
            r.url=phase.equals("running")?g.optString("url"):"";
            r.state.setText(phase.equals("running")?"Local and public ready":phase.equals("connecting")?"Local ready · Public connecting":phase.equals("starting")?"Starting…":phase.equals("stopping")?"Saving and stopping…":phase.equals("error")?"Needs attention":"Stopped");
            r.detail.setText(fresh?g.optString("message","Start this game independently."):"Host stopped. Saved games remain on this phone.");
            r.start.setEnabled(!running&&!checking);r.stop.setEnabled(running&&!phase.equals("stopping"));r.open.setEnabled(enabled);r.share.setEnabled(!r.url.isEmpty());r.copy.setEnabled(!r.url.isEmpty());r.link.setText(r.url.isEmpty()?"Public link not verified yet.":r.url);
        }
        both.setEnabled(!checking&&rows.values().stream().anyMatch(r->r.phase.equals("stopped")||r.phase.equals("error")));stopAll.setEnabled(active||checking);settings.setEnabled(!active&&!checking);tunnel.setText(fresh?status.optString("tunnel","Stopped"):"Shared host stopped");
        JSONObject update=read("update-state.json");updates.setText(checking?"Checking GitHub before Start…":update.optString("message","Checks GitHub before every Start.")+"\nBuild "+BuildConfig.VERSION_NAME+(BuildConfig.SOURCE_DIRTY?" · Unpublished local changes":""));review.setVisibility(!checking&&!update.optString("url").isEmpty()?View.VISIBLE:View.GONE);
        JSONObject result=read("sync-summary.json");if(!result.has("updated"))result=read("sync-result.json");if(result.has("updated"))sync.setText(result.optString("message")+"\nSync waits for both games on both hosts to stop.");
    }
    private void check(String game){if(checking)return;checking=true;target=game;pending=null;int current=++generation;refresh();checkThread=new Thread(()->{GitHubUpdates.Result result=new GitHubUpdates().check();handler.post(()->{if(isDestroyed()||isFinishing()||current!=generation)return;checking=false;try{Files.write(new File(getFilesDir(),"update-state.json").toPath(),result.json().toString().getBytes(StandardCharsets.UTF_8));}catch(Exception ignored){}refresh();if(resumed)finishCheck(result,game);else{pending=result;target=game;}});},"github-check");checkThread.start();}
    private void cancelCheck(){++generation;checking=false;pending=null;if(checkThread!=null)checkThread.interrupt();refresh();}
    private void finishCheck(GitHubUpdates.Result result,String game){
        if(result.failed)new AlertDialog.Builder(this).setTitle("Couldn’t complete GitHub check").setMessage(result.message).setPositiveButton("Retry",(d,w)->check(game)).setNeutralButton("Start installed",(d,w)->launch(game)).setNegativeButton("Cancel",null).show();
        else if(result.installable())startUpdate(result,game);
        else if(result.news)new AlertDialog.Builder(this).setTitle("GitHub update available").setMessage(result.message+"\n\nStop both games before installing an APK. Reviewing this link does not stop a running game.").setPositiveButton("Review update",(d,w)->open(result.url)).setNeutralButton("Start installed",(d,w)->launch(game)).setNegativeButton("Cancel",null).show();
        else launch(game);
    }
    /** Newer verified APK found on Start: download, verify, stop games cleanly, install, restart. */
    private void startUpdate(GitHubUpdates.Result update,String game){
        if(Build.VERSION.SDK_INT>=26&&!getPackageManager().canRequestPackageInstalls()){
            new AlertDialog.Builder(this).setTitle("Allow Go Host updates").setMessage("Go Host "+update.version+" is ready on GitHub.\n\nAndroid needs a one-time permission before Go Host can install its own updates. Allow it, then return here and the update continues.")
                .setPositiveButton("Open settings",(d,w)->{pendingUpdate=update;pendingUpdateGame=game;startActivity(new Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,Uri.parse("package:"+getPackageName())));})
                .setNeutralButton("Start installed",(d,w)->launch(game)).setNegativeButton("Cancel",null).show();
            return;
        }
        Set<String> running=new LinkedHashSet<>();for(Row r:rows.values())if(!r.phase.equals("stopped")&&!r.phase.equals("error"))running.add(r.id);
        if(game.equals("both")){running.add("dominogo");running.add("unogo");}else running.add(game);
        String resume=running.size()>1?"both":running.iterator().next();
        LinearLayout box=column();box.setPadding(dp(24),dp(8),dp(24),dp(8));TextView status=label("Downloading Go Host "+update.version+"…",15,text);box.addView(status);
        ProgressBar bar=new ProgressBar(this,null,android.R.attr.progressBarStyleHorizontal);bar.setMax(1000);bar.setIndeterminate(true);add(box,bar,12);
        add(box,label(active?"Running games are saved, stopped for the update and start again automatically.":"Your games start automatically after the update.",13,secondary),12);
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle("Updating Go Host").setView(box).setCancelable(false)
            .setNegativeButton("Cancel",(d,w)->{if(updateThread!=null)updateThread.interrupt();ApkUpdater.writeState(this,"Update canceled. Installed version "+BuildConfig.VERSION_NAME+".",false);}).create();
        dialog.show();
        updateThread=new Thread(()->{
            try{
                File apk=ApkUpdater.download(this,update,(done,total)->handler.post(()->{if(total>0){bar.setIndeterminate(false);bar.setProgress((int)(done*1000/total));}status.setText("Downloading Go Host "+update.version+"… "+(done/1_048_576)+" MB");}));
                handler.post(()->status.setText("Verified. Saving and stopping games…"));
                if(Thread.currentThread().isInterrupted())throw new InterruptedException();
                ApkUpdater.rememberResume(this,resume,update.version);
                handler.post(()->stop("both"));
                long until=System.currentTimeMillis()+30_000;
                while(System.currentTimeMillis()<until&&hostRunning()){Thread.sleep(500);}
                if(Thread.currentThread().isInterrupted())throw new InterruptedException();
                handler.post(()->status.setText("Installing…"));
                ApkUpdater.install(this,apk);
                handler.post(()->{if(dialog.isShowing())dialog.dismiss();});
            }catch(InterruptedException|InterruptedIOException e){
                // Canceled. If the games were already stopped for the update, start them again.
                String back=ApkUpdater.takeResume(this);
                if(!back.isEmpty())handler.post(()->launch(back));
            }catch(Exception e){
                String why=e.getMessage()==null?"The update failed.":e.getMessage();
                ApkUpdater.writeState(this,"Update failed: "+why+" Installed version "+BuildConfig.VERSION_NAME+" kept.",true);
                String back=ApkUpdater.takeResume(this);
                handler.post(()->{if(dialog.isShowing())dialog.dismiss();if(isFinishing())return;
                    new AlertDialog.Builder(this).setTitle("Update not installed").setMessage(why+"\n\nThe installed version is unchanged.").setPositiveButton("Start installed",(d,w)->launch(back.isEmpty()?game:back)).setNegativeButton("Close",null).show();});
            }
        },"apk-update");
        updateThread.start();
    }
    /** True while the hosting service still reports a game that is not stopped. */
    private boolean hostRunning(){
        JSONObject status=read("host-state.json"),games=status.optJSONObject("games");
        if(games==null||System.currentTimeMillis()-status.optLong("updated")>=12000)return false;
        for(Iterator<String> it=games.keys();it.hasNext();){JSONObject g=games.optJSONObject(it.next());String phase=g==null?"stopped":g.optString("phase","stopped");if(!phase.equals("stopped")&&!phase.equals("error"))return true;}
        return false;
    }
    private void launch(String game){if(Build.VERSION.SDK_INT>=33&&checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=android.content.pm.PackageManager.PERMISSION_GRANTED)requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},1);startForegroundService(new Intent(this,HostService.class).setAction(HostService.START).putExtra("game",game));handler.postDelayed(this::refresh,350);}
    private void stop(String game){if(active)startService(new Intent(this,HostService.class).setAction(game.equals("both")?HostService.STOP:HostService.STOP_GAME).putExtra("game",game));}
    private void open(String url){try{startActivity(new Intent(Intent.ACTION_VIEW,Uri.parse(url)));}catch(ActivityNotFoundException e){new AlertDialog.Builder(this).setMessage("Install a browser to open this link.").setPositiveButton("OK",null).show();}}
    private void settings(){
        SharedPreferences prefs=getSharedPreferences("hosting",MODE_PRIVATE);LinearLayout box=column();box.setPadding(dp(24),dp(8),dp(24),dp(8));box.addView(label("Keep the dedicated game tunnel. Cloudflare needs DominoGo → localhost:3000 and UnoGo → localhost:3001. Empty token uses separate quick links.",15,secondary));
        EditText domino=field(box,"DominoGo HTTPS hostname",prefs.getString("hostname",""),false),uno=field(box,"UnoGo HTTPS hostname",prefs.getString("unoHostname","https://uno.driveatbaku.games"),false),token=field(box,"Shared tunnel token",prefs.getString("token",""),true);
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle("Public links").setView(box).setNegativeButton("Cancel",null).setPositiveButton("Save",null).create();
        dialog.setOnShowListener(d->{dialog.getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v->{String a=domino.getText().toString().trim().replaceAll("/+$",""),b=uno.getText().toString().trim().replaceAll("/+$",""),t=token.getText().toString().trim();if(!t.isEmpty()||!a.isEmpty()){if(!host(a)||!host(b)||a.equals(b)||t.isEmpty()||t.matches(".*\\s.*")){domino.setError("Use two distinct HTTPS hostnames and their shared tunnel token.");return;}}else if(!b.isEmpty()&&!host(b)){uno.setError("Use an HTTPS hostname or leave it empty.");return;}prefs.edit().putString("hostname",a).putString("unoHostname",b).putString("token",t).apply();dialog.dismiss();});});dialog.show();
    }
    private EditText field(LinearLayout box,String hint,String value,boolean secret){box.addView(label(hint,14,secondary));EditText f=new EditText(this);f.setSingleLine();f.setText(value);f.setHint(hint);f.setInputType(InputType.TYPE_CLASS_TEXT|(secret?InputType.TYPE_TEXT_VARIATION_PASSWORD:InputType.TYPE_TEXT_VARIATION_URI));add(box,f,4);return f;}
    private static boolean host(String value){try{URI u=new URI(value);return "https".equals(u.getScheme())&&u.getHost()!=null&&u.getUserInfo()==null&&u.getQuery()==null&&u.getFragment()==null&&u.getPort()==-1&&u.getPath().isEmpty();}catch(Exception e){return false;}}
    private void logs(){TextView content=label(read("host-state.json").optString("logs","No connection details yet."),13,text);content.setTextIsSelectable(true);content.setPadding(dp(20),dp(12),dp(20),dp(12));ScrollView s=new ScrollView(this);s.addView(content);new AlertDialog.Builder(this).setTitle("Connection details").setView(s).setPositiveButton("Done",null).show();}
    protected void onResume(){super.onResume();resumed=true;handler.post(poll);
        if(pendingUpdate!=null&&(Build.VERSION.SDK_INT<26||getPackageManager().canRequestPackageInstalls())){GitHubUpdates.Result u=pendingUpdate;String g=pendingUpdateGame;pendingUpdate=null;handler.post(()->{if(resumed&&!isFinishing())startUpdate(u,g);});}if(pending!=null){GitHubUpdates.Result result=pending;String game=target;pending=null;handler.post(()->{if(resumed&&!isFinishing())finishCheck(result,game);});}}
    protected void onPause(){resumed=false;handler.removeCallbacks(poll);super.onPause();}
    protected void onDestroy(){cancelCheck();super.onDestroy();}
}
