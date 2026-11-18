"""Physical Android acceptance using a test-only WebView and OS touch injection.

The server and accounts are isolated. No player database is opened or changed.
Install the androidTest APK first. Requires the explicitly selected ADB serial.
"""
import argparse
import json
import sqlite3
import subprocess
import threading
import time

import browser_flow as flow
from live_host import ROOT, api


def main():
    args=argparse.ArgumentParser();args.add_argument('--serial',required=True);serial=args.parse_args().serial
    adb=['adb','-s',serial]
    flow.PUBLIC=ROOT/'unogo/target/dx/uno-client/release/web/public'
    flow.BINARY=ROOT/'rust/target/release/domino-server.exe'
    host=flow.Host();failure=[];shots=ROOT/'unogo/docs/screenshots/device-table';shots.mkdir(parents=True,exist_ok=True)
    try:
        owner=api(host.uno,'/api/auth/guest',body={'prefix':'DeviceCheck'})
        code=api(host.uno,'/api/lobbies',owner['token'],{'maxPlayers':2})['id']
        subprocess.run(adb+['reverse','tcp:3311',f'tcp:{host.uno}'],check=True,capture_output=True)
        def prepare():
            try:
                for _ in range(150):
                    if len(api(host.uno,f'/api/lobbies/{code}',owner['token'])['players'])==2:break
                    time.sleep(.2)
                else:raise AssertionError('Phone did not join the isolated room')
                api(host.uno,f'/api/lobbies/{code}/start',owner['token'],{})
                with sqlite3.connect(host.database) as db:
                    raw=json.loads(db.execute('SELECT state_json FROM uno_tables WHERE id=?',(code,)).fetchone()[0]);g=raw['game']
                    pool=g['draw']+[c for h in g['hands'] for c in h]+g['discard']
                    def take(pred):
                        card=next(c for c in pool if pred(c));pool.remove(card);return card
                    top=take(lambda c:c['color']=='Red' and c['value']=={'Number':3})
                    legal=take(lambda c:c['color']=='Red' and c['value']=={'Number':5})
                    red8=take(lambda c:c['color']=='Red' and c['value']=={'Number':8})
                    blue8=take(lambda c:c['color']=='Blue' and c['value']=={'Number':8})
                    g['hands']=[pool[:7],[legal,red8,blue8]+pool[7:11]];g['draw']=pool[11:];g['discard']=[top]
                    now=int(time.time()*1000);g.update(turn=1,color='Red',phase='Turn',deadline=now+600000,last_time=now,pending=None)
                    assert sum(map(len,g['hands']))+len(g['draw'])+len(g['discard'])==108
                    db.execute('UPDATE uno_tables SET state_json=?,saved_at=? WHERE id=?',(json.dumps(raw),now,code))
                db.close()
            except Exception as e:failure.append(str(e))
        thread=threading.Thread(target=prepare);thread.start()
        process=subprocess.Popen(adb+['shell','am','instrument','-w','-e','url',f'http://127.0.0.1:3311/?join={code}','games.dominogo.host.test/games.dominogo.host.UpdateCheckInstrumentation'],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
        output=[]
        try:
            for line in process.stdout:
                output.append(line)
                if 'TOUCH_STAGE=' in line:
                    stage=line.strip().split('TOUCH_STAGE=')[-1]
                    if stage in ['before-drag','held-over-pile','after-drag','five-tap-ack','five-result','own-and-opponent-emotes','direct-number-selection','after-number-group']:
                        image=subprocess.check_output(adb+['exec-out','screencap','-p'],timeout=8)
                        (shots/f'{stage}.png').write_bytes(image)
                    if stage=='five-tap-ack':
                        state=api(host.uno,f'/api/lobbies/{code}',owner['token'])
                        challenge=state['game']['phase']['TouchFive']['challenge']
                        api(host.uno,f'/api/lobbies/{code}/commands',owner['token'],{'matchId':state['matchId'],'sequence':state['acceptedSequence']+1,'action':{'TouchFive':{'challenge':challenge}}})
                    if stage=='five-result':
                        api(host.uno,f'/api/lobbies/{code}/reactions',owner['token'],{'requestId':'device-visual-emote','emoji':'🎉'})
                    if stage=='own-and-opponent-emotes':
                        state=api(host.uno,f'/api/lobbies/{code}',owner['token'])
                        api(host.uno,f'/api/lobbies/{code}/commands',owner['token'],{'matchId':state['matchId'],'sequence':state['acceptedSequence']+1,'action':'Draw'})
                        state=api(host.uno,f'/api/lobbies/{code}',owner['token'])
                        if isinstance(state['game']['phase'],dict) and 'Drawn' in state['game']['phase']:
                            api(host.uno,f'/api/lobbies/{code}/commands',owner['token'],{'matchId':state['matchId'],'sequence':state['acceptedSequence']+1,'action':'Keep'})
                    print(stage,flush=True)
            process.wait(timeout=15);thread.join(timeout=3)
        finally:
            if process.poll() is None:process.terminate()
        assert not failure,failure
        assert 'PASS: physical Android touch drag' in ''.join(output),''.join(output)[-2400:]
        with sqlite3.connect(host.database) as db:
            raw=json.loads(db.execute('SELECT state_json FROM uno_tables WHERE id=?',(code,)).fetchone()[0]);assert len(raw['game']['hands'][1])==4
            assert raw['game']['discard'][-1]['color']=='Blue'
            assert raw['game']['discard'][-1]['value']=={'Number':8}
            assert raw['game']['last_touch']['order']==[1,0]
            assert raw['game']['last_touch']['penalty']==0
            assert len(raw['game']['hands'][0])==9
        db.close()
        print('PASS: physical Galaxy S9 OS drag, 5 tap, own/remote emotes and direct number grouping/final color; isolated data only',flush=True)
    finally:
        subprocess.run(adb+['reverse','--remove','tcp:3311'],capture_output=True)
        host.close()


if __name__=='__main__':main()
