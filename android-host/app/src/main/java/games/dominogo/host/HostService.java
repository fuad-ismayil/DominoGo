package games.dominogo.host;

import android.app.*;
import java.lang.Process;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.net.wifi.WifiManager;
import android.os.*;
import org.json.JSONObject;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.regex.*;

/** One private database/runtime; independently controlled games and shared tunnel. */
public class HostService extends Service {
    static final String START = "games.dominogo.host.START", STOP_GAME = "games.dominogo.host.STOP_GAME", STOP = "games.dominogo.host.STOP";
    private static native String nativeRun(String database, String web, String unoWeb);
    private static native String nativeSetEnabled(String game, boolean enabled);
    private static native String nativeStatus();
    private static native void nativeStop();
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ExecutorService commands = Executors.newSingleThreadExecutor();
    private final AtomicBoolean stopping = new AtomicBoolean();
    private final Object processLock = new Object();
    private final LinkedHashMap<String, GameRow> games = new LinkedHashMap<>();
    private volatile Thread engine;
    private Process namedTunnel;
    private PowerManager.WakeLock wake;
    private WifiManager.WifiLock wifi;
    private RandomAccessFile lockFile;
    private java.nio.channels.FileLock databaseLock;
    private volatile String tunnelState = "Stopped", logs = "", hostId = "";
    private String tunnelToken = "", dominoHost = "", unoHost = "";
    private static class GameRow {
        final String id, name; final int port;
        volatile boolean desired, enabled;
        volatile String phase = "stopped", message = "Ready to host", url = "";
        volatile Process quickTunnel;
        GameRow(String id, String name, int port) { this.id=id; this.name=name; this.port=port; }
    }
    private final Runnable heartbeat = new Runnable() { public void run() { save(); handler.postDelayed(this,2000); } };

    public void onCreate() {
        super.onCreate(); games.put("dominogo",new GameRow("dominogo","DominoGo",3000)); games.put("unogo",new GameRow("unogo","UnoGo",3001));
        getSystemService(NotificationManager.class).createNotificationChannel(new NotificationChannel("hosting","Game hosting",NotificationManager.IMPORTANCE_LOW));
        Notification n=notification("Preparing the host…");
        if(Build.VERSION.SDK_INT>=34) startForeground(1,n,ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE); else startForeground(1,n);
        wake=getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK,"GoHost:Hosting");wake.acquire();
        wifi=((WifiManager)getApplicationContext().getSystemService(WIFI_SERVICE)).createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF,"GoHost:Hosting");wifi.acquire();
        handler.post(heartbeat);
        new Thread(this::monitor,"host-health").start();
    }
    private Notification notification(String detail) {
        PendingIntent open=PendingIntent.getActivity(this,0,new Intent(this,MainActivity.class),PendingIntent.FLAG_IMMUTABLE|PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stop=PendingIntent.getService(this,1,new Intent(this,HostService.class).setAction(STOP),PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this,"hosting").setSmallIcon(R.drawable.host_icon).setContentTitle("Go Host · DominoGo + UnoGo").setContentText(detail).setContentIntent(open).setOngoing(true)
            .addAction(new Notification.Action.Builder(null,"Stop all",stop).build()).build();
    }
    public int onStartCommand(Intent intent,int flags,int startId) {
        if(intent==null) {finishHosting("");return START_NOT_STICKY;}
        if(STOP.equals(intent.getAction())) {for(GameRow g:games.values())g.desired=false;finishHosting("");return START_NOT_STICKY;}
        String which=intent.getStringExtra("game"); if(which==null)which="dominogo";
        boolean enable=!STOP_GAME.equals(intent.getAction());
        List<GameRow> targets=new ArrayList<>();
        if(which.equals("both"))targets.addAll(games.values());else if(games.containsKey(which))targets.add(games.get(which));
        for(GameRow game:targets) {game.desired=enable;game.phase=enable?"starting":"stopping";game.message=enable?"Starting installed game…":"Saving and stopping this game…";}
        save();
        commands.execute(()->{
            try {
                for(GameRow game:targets) {
                    if(stopping.get())return;
                    if(game.desired) {
                        ensureEngine(); if(!game.desired||stopping.get())continue;
                        String error=nativeSetEnabled(game.id,true);if(!error.isEmpty())throw new IOException(error);
                        game.enabled=true; game.phase="connecting";game.message="Local game ready. Checking public route…";
                        if(tunnelToken.isEmpty())ensureQuick(game);else ensureNamed();
                    } else {
                        if(engine!=null&&engine.isAlive()) {String error=nativeSetEnabled(game.id,false);if(!error.isEmpty())throw new IOException(error);}
                        game.enabled=false;game.url="";game.phase="stopped";game.message="Stopped. Saved games are preserved.";destroy(game.quickTunnel);game.quickTunnel=null;
                    }
                }
                save();
                if(games.values().stream().noneMatch(g->g.desired||g.enabled))finishHosting("");
            }catch(Throwable e){for(GameRow game:targets){game.phase="error";game.message=message(e);}save();if(games.values().stream().noneMatch(g->g.enabled))finishHosting(message(e));}
        });
        return START_NOT_STICKY;
    }
    private void ensureEngine() throws Exception {
        if(engine!=null) {if(!engine.isAlive())throw new IOException("Game runtime exited. Stop all and restart.");return;}
        lockFile=new RandomAccessFile(new File(getFilesDir(),"database.lock"),"rw");databaseLock=lockFile.getChannel().tryLock();
        if(databaseLock==null)throw new IOException("Database sync is in progress. Wait and retry.");
        File domino=installAssets("web"),uno=installAssets("web-unogo");
        android.content.SharedPreferences prefs=getSharedPreferences("hosting",MODE_PRIVATE);
        tunnelToken=prefs.getString("token","");dominoHost=prefs.getString("hostname","");unoHost=prefs.getString("unoHostname","https://uno.driveatbaku.games");
        System.loadLibrary("domino_server");
        synchronized(processLock){if(stopping.get())return;engine=new Thread(()->{
            String error=nativeRun(new File(getFilesDir(),"database.sqlite").getAbsolutePath(),domino.getAbsolutePath(),uno.getAbsolutePath());
            if(!stopping.get())finishHosting(error==null||error.isEmpty()?"Game runtime exited. Tap Start to retry.":error);
        },"rust-shared-host");engine.start();}
        for(int i=0;i<80&&!stopping.get();i++){JSONObject status=new JSONObject(nativeStatus());if(status.has("dominogo")){hostId=status.optString("hostId");return;}if(!engine.isAlive())throw new IOException("Game runtime exited during startup");Thread.sleep(250);}
        if(!stopping.get())throw new IOException("Host did not become ready. Stop all and retry.");
    }
    private File installAssets(String name) throws Exception {
        File web=new File(getFilesDir(),name),marker=new File(web,".installed");String version=""+getPackageManager().getPackageInfo(getPackageName(),0).lastUpdateTime;
        if(!marker.exists()||!new String(Files.readAllBytes(marker.toPath()),StandardCharsets.UTF_8).equals(version)){copyAssets(name,web);Files.write(marker.toPath(),version.getBytes(StandardCharsets.UTF_8));}
        return web;
    }
    private void copyAssets(String name,File output)throws IOException{
        if(stopping.get())throw new IOException("Stopped");String[] children=getAssets().list(name);
        if(children!=null&&children.length>0){if(!output.isDirectory()&&!output.mkdirs())throw new IOException("Cannot create game directory");for(String child:children)copyAssets(name+"/"+child,new File(output,child));}
        else try(InputStream in=getAssets().open(name);OutputStream out=new FileOutputStream(output)){byte[] bytes=new byte[32768];int n;while((n=in.read(bytes))>0)out.write(bytes,0,n);}
    }
    private Process tunnel(List<String> extra, String configName)throws Exception{
        List<String> args=new ArrayList<>(Arrays.asList(new File(getApplicationInfo().nativeLibraryDir,"libcloudflared.so").getAbsolutePath(),"tunnel","--config",new File(getFilesDir(),configName).getAbsolutePath(),"--no-autoupdate","--protocol","http2","--edge-ip-version","auto"));args.addAll(extra);
        ProcessBuilder pb=new ProcessBuilder(args).directory(getFilesDir()).redirectErrorStream(true);pb.environment().put("HOME",getFilesDir().getAbsolutePath());pb.environment().put("TMPDIR",getCacheDir().getAbsolutePath());
        if(!tunnelToken.isEmpty())pb.environment().put("TUNNEL_TOKEN",tunnelToken);
        android.net.ConnectivityManager cm=getSystemService(android.net.ConnectivityManager.class);android.net.LinkProperties properties=cm.getLinkProperties(cm.getActiveNetwork());
        if(properties!=null){StringBuilder dns=new StringBuilder();for(InetAddress server:properties.getDnsServers())dns.append(server.getHostAddress()).append(' ');pb.environment().put("DOMINO_DNS_SERVERS",dns.toString().trim());}
        synchronized(processLock){if(stopping.get())throw new IOException("Stopped");return pb.start();}
    }
    private synchronized void ensureNamed()throws Exception{
        synchronized(processLock){if(namedTunnel!=null&&namedTunnel.isAlive())return;}
        // Token-managed tunnels also need both ingress routes in Cloudflare's
        // configuration. This local file never changes that remote account.
        String yaml="no-autoupdate: true\ningress:\n  - hostname: "+new URL(dominoHost).getHost()+"\n    service: http://127.0.0.1:3000\n  - hostname: "+new URL(unoHost).getHost()+"\n    service: http://127.0.0.1:3001\n  - service: http_status:404\n";
        Files.write(new File(getFilesDir(),"shared-tunnel.yml").toPath(),yaml.getBytes(StandardCharsets.UTF_8));
        namedTunnel=tunnel(Collections.singletonList("run"),"shared-tunnel.yml");readTunnel(namedTunnel,null);tunnelState="Connecting shared named tunnel";
    }
    private synchronized void ensureQuick(GameRow game)throws Exception{
        if(game.quickTunnel!=null&&game.quickTunnel.isAlive())return;
        String config="quick-"+game.id+".yml";Files.write(new File(getFilesDir(),config).toPath(),"no-autoupdate: true\n".getBytes(StandardCharsets.UTF_8));
        game.quickTunnel=tunnel(Arrays.asList("--url","http://127.0.0.1:"+game.port),config);readTunnel(game.quickTunnel,game);tunnelState="Quick links (one per game)";
    }
    private void readTunnel(Process process,GameRow game){new Thread(()->{
        Pattern link=Pattern.compile("https://[a-z0-9-]+\\.trycloudflare\\.com");
        try(BufferedReader reader=new BufferedReader(new InputStreamReader(process.getInputStream(),StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){log(tunnelToken.isEmpty()?line:line.replace(tunnelToken,"[hidden]"));Matcher m=link.matcher(line);if(game!=null&&m.find()&&game.desired)game.url=m.group();}}
        catch(IOException e){if(!stopping.get())log(message(e));}
    },"tunnel-output").start();}
    private void monitor(){while(!stopping.get()){
        try{
            if(engine!=null&&engine.isAlive()&&!hostId.isEmpty()){JSONObject status=new JSONObject(nativeStatus());for(GameRow g:games.values())g.enabled=status.optBoolean(g.id);}
            boolean any=games.values().stream().anyMatch(g->g.enabled);
            if(any&&!tunnelToken.isEmpty()){try{ensureNamed();}catch(Exception e){tunnelState="Reconnecting: "+message(e);}}
            for(GameRow game:games.values()){
                if(!game.enabled||!game.desired)continue;
                boolean local=healthy("http://127.0.0.1:"+game.port+"/api/health",game.id);
                if(!game.enabled||!game.desired)continue;
                if(!local){game.phase="error";game.message="Local game is not responding. Stop this game and retry.";continue;}
                if(tunnelToken.isEmpty()){try{ensureQuick(game);}catch(Exception e){game.phase="connecting";game.message="Local game ready; public connection failed. Retrying…";continue;}}
                else game.url=game.id.equals("dominogo")?dominoHost:unoHost;
                boolean publicReady=!game.url.isEmpty()&&healthy(game.url+"/api/health",game.id);
                if(!game.enabled||!game.desired)continue;
                if(publicReady){game.phase="running";game.message="Local and public game ready. Share the link.";}
                else{game.phase="connecting";game.message=tunnelToken.isEmpty()?"Local game ready. Waiting for the quick link…":"Local game ready. Public route unavailable; check the game's hostname in Cloudflare.";}
            }
            save();Thread.sleep(4000);
        }catch(InterruptedException e){return;}catch(Throwable e){if(!stopping.get())log(message(e));}
    }}
    private boolean healthy(String address,String game){HttpURLConnection c=null;try{
        c=(HttpURLConnection)new URL(address).openConnection();c.setConnectTimeout(3000);c.setReadTimeout(3000);if(c.getResponseCode()!=200)return false;
        try(InputStream in=c.getInputStream()){ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] bytes=new byte[4096];int n;while((n=in.read(bytes))>0){if(out.size()+n>65536)return false;out.write(bytes,0,n);}JSONObject status=new JSONObject(out.toString("UTF-8"));return game.equals(status.optString("gameId"))&&!hostId.isEmpty()&&hostId.equals(status.optString("hostId"));}
    }catch(Exception e){return false;}finally{if(c!=null)c.disconnect();}}
    private static String message(Throwable e){return e.getMessage()==null?e.toString():e.getMessage();}
    private synchronized void log(String line){logs+=line+"\n";if(logs.length()>16000)logs=logs.substring(logs.length()-16000);}
    private synchronized void save(){try{
        JSONObject rows=new JSONObject();int count=0;for(GameRow g:games.values()){if(g.enabled)count++;rows.put(g.id,new JSONObject().put("phase",g.phase).put("message",g.message).put("enabled",g.enabled).put("url",g.phase.equals("running")?g.url:"").put("localUrl","http://127.0.0.1:"+g.port));}
        String summary=count+" game(s) enabled";
        JSONObject status=new JSONObject().put("phase",stopping.get()?"stopped":count>0?"running":"starting").put("message",summary).put("games",rows).put("tunnel",tunnelState).put("logs",logs).put("updated",System.currentTimeMillis());
        File temp=new File(getFilesDir(),"host-state.tmp");Files.write(temp.toPath(),status.toString().getBytes(StandardCharsets.UTF_8));Files.move(temp.toPath(),new File(getFilesDir(),"host-state.json").toPath(),StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);
        if(!stopping.get())getSystemService(NotificationManager.class).notify(1,notification(summary+" · "+tunnelState));
    }catch(Exception ignored){}}
    private static void destroy(Process process){if(process==null)return;process.destroy();try{if(!process.waitFor(2,TimeUnit.SECONDS))process.destroyForcibly();}catch(InterruptedException e){process.destroyForcibly();}}
    private void finishHosting(String error){if(!stopping.compareAndSet(false,true))return;
        for(GameRow g:games.values()){g.desired=false;g.phase=error.isEmpty()?"stopped":"error";g.message=error.isEmpty()?"Stopped. Saved games are preserved.":error;g.url="";}
        handler.removeCallbacks(heartbeat);commands.shutdown();
        new Thread(()->{
            try{if(engine!=null&&engine.isAlive()){nativeSetEnabled("dominogo",false);nativeSetEnabled("unogo",false);nativeStop();engine.join(5000);}}catch(Throwable ignored){}
            synchronized(processLock){destroy(namedTunnel);for(GameRow g:games.values()){destroy(g.quickTunnel);g.enabled=false;}}
            tunnelState="Stopped";save();
            handler.post(()->{if(wake!=null&&wake.isHeld())wake.release();if(wifi!=null&&wifi.isHeld())wifi.release();stopForeground(STOP_FOREGROUND_REMOVE);stopSelf();
                // Keep database.lock held until the process exits: a timeout must
                // never unlock a database that an engine could still be using.
                android.os.Process.killProcess(android.os.Process.myPid());});
        },"host-stop-all").start();
    }
    public void onDestroy(){finishHosting("");super.onDestroy();}
    public IBinder onBind(Intent intent){return null;}
}
