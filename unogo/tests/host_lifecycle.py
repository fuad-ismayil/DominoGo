"""No phone required: prove independent lifecycle on the actual shared desktop host."""
import contextlib,json,os,secrets,sqlite3,subprocess,tempfile,time,urllib.request,urllib.error
from pathlib import Path
from live_host import ROOT,api,free_port

def main():
    executable=ROOT/"rust/target/release"/("domino-server.exe" if os.name=="nt" else "domino-server")
    with tempfile.TemporaryDirectory(prefix="go-host-lifecycle-") as directory:
        directory=Path(directory);database=directory/"database.sqlite"
        domino,uno,control=free_port(),free_port(),free_port();secret=secrets.token_hex(32)
        env=dict(os.environ,DOMINO_DB_PATH=str(database),DOMINO_PORT=str(domino),UNOGO_PORT=str(uno),GO_HOST_CONTROL_PORT=str(control),GO_HOST_CONTROL_TOKEN=secret)
        process=None
        def start(initial="both"):
            nonlocal process
            env["GO_HOST_INITIAL_GAMES"]=initial
            with (directory/"host.log").open("ab") as log:
                process=subprocess.Popen([str(executable)],cwd=directory,env=env,stdout=log,stderr=log,creationflags=subprocess.CREATE_NO_WINDOW if os.name=="nt" else 0)
            for _ in range(100):
                if process.poll() is not None:raise AssertionError((directory/"host.log").read_text())
                try:api(control,"/status",secret);return
                except OSError:time.sleep(.1)
            raise AssertionError("Control did not become ready")
        def stop():
            if process and process.poll() is None:process.terminate();process.wait(timeout=10)
        def saved(table,column,id):
            with contextlib.closing(sqlite3.connect(database)) as db:return db.execute(f"SELECT {column} FROM {table} WHERE "+("id" if table=="uno_tables" else "lobby_id")+"=?",(id,)).fetchone()[0]
        def enabled(game,value):return api(control,f"/games/{game}",secret,{"enabled":value})
        try:
            start()
            api(domino,"/api/health");api(uno,"/api/health")
            try:urllib.request.urlopen(f"http://127.0.0.1:{control}/status",timeout=2);raise AssertionError("Control needs authentication")
            except urllib.error.HTTPError as e:assert e.code==401
            a=api(domino,"/api/auth/guest",body={"prefix":"Host"});b=api(uno,"/api/auth/guest",body={"prefix":"Friend"})
            d=api(domino,"/api/lobbies",a["token"],{"maxPlayers":2})["id"]
            api(domino,f"/api/lobbies/{d}/add-bot",a["token"],{});api(domino,f"/api/lobbies/{d}/start",a["token"],{})
            u=api(uno,"/api/lobbies",a["token"],{"maxPlayers":2})["id"]
            api(uno,f"/api/lobbies/{u}/join",b["token"],{});state=api(uno,f"/api/lobbies/{u}/start",a["token"],{})
            enabled("unogo",False);assert api(uno,"/api/health",expected=503)["code"]=="GAME_OFFLINE";api(domino,"/api/health")
            frozen=saved("uno_tables","state_json",u);assert json.loads(frozen)["host_paused"]
            time.sleep(1.8);assert saved("uno_tables","state_json",u)==frozen
            enabled("unogo",True);restored=api(uno,f"/api/lobbies/{u}",a["token"])
            assert restored["game"]["hand"]==state["game"]["hand"] and not restored["game"]["paused"]
            enabled("dominogo",False);api(domino,"/api/health",expected=503);api(uno,"/api/health")
            frozen_domino=saved("game_states","state_json",d);time.sleep(1.8);assert saved("game_states","state_json",d)==frozen_domino
            enabled("dominogo",True);assert api(domino,f"/api/lobbies/{d}/join",a["token"],{})["status"]=="playing"
            enabled("unogo",False);enabled("dominogo",False)
            assert api(control,"/status",secret)=={"dominogo":False,"unogo":False}
            stop();start("none");api(domino,"/api/health",expected=503);api(uno,"/api/health",expected=503)
            enabled("unogo",True);api(uno,"/api/health");api(domino,"/api/health",expected=503)
            assert api(uno,f"/api/lobbies/{u}",a["token"])["game"]["hand"]==state["game"]["hand"]
            print("PASS: authenticated loopback control, independent Stop/Start, frozen hands/timers/bots, both-stopped recovery, shared accounts retained")
        finally:stop()

if __name__=="__main__":main()
