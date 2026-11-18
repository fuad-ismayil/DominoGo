"""Verify live-table geometry with isolated accounts and private snapshot fixtures."""
import copy
import json
import re
import sqlite3
import time

from playwright.sync_api import sync_playwright, expect

import browser_flow as flow
from live_host import ROOT, api


def main():
    flow.PUBLIC = ROOT / "unogo/target/dx/uno-client/release/web/public"
    flow.BINARY = ROOT / "rust/target/release/domino-server.exe"
    host = flow.Host()
    shots = ROOT / "unogo/docs/screenshots/game-viewport"
    shots.mkdir(parents=True, exist_ok=True)
    try:
        user = api(host.uno, "/api/auth/guest", body={"prefix": "PhonePlayer"})
        code = api(host.uno, "/api/lobbies", user["token"], {"maxPlayers": 10})["id"]
        for _ in range(9):
            api(host.uno, f"/api/lobbies/{code}/add-bot", user["token"], {})
        api(host.uno, f"/api/lobbies/{code}/start", user["token"], {})
        # Conserved 40-card hand, confined to this temporary database.
        with sqlite3.connect(host.database) as db:
            raw = json.loads(db.execute("SELECT state_json FROM uno_tables WHERE id=?", (code,)).fetchone()[0])
            game = raw["game"]
            pool = game["draw"] + [c for hand in game["hands"] for c in hand] + game["discard"][:-1]
            assert len(pool) == 107 and len({c["id"] for c in pool}) == 107
            game["hands"] = [pool[:40]] + [pool[40+i*7:47+i*7] for i in range(9)]
            game["draw"] = pool[103:]
            game["discard"] = game["discard"][-1:]
            now = int(time.time()*1000)
            game.update(turn=0, phase="Turn", deadline=now+600000, last_time=now, pending=None)
            db.execute("UPDATE uno_tables SET state_json=?,saved_at=?,bot_due=? WHERE id=?",
                       (json.dumps(raw), now, now+600000, code))
        db.close()
        snapshot = api(host.uno, f"/api/lobbies/{code}", user["token"])
        current = copy.deepcopy(snapshot)
        with sync_playwright() as p:
            browser = p.chromium.launch(channel="msedge", headless=True)
            try:
                for width, height, top, bottom, name in [
                    (393,852,59,34,"iphone"), (390,664,0,0,"browser-phone"),
                    (320,568,20,0,"small-phone"), (852,393,0,21,"landscape"),
                    (1440,960,0,0,"desktop"),
                ]:
                    context = browser.new_context(viewport={"width":width,"height":height})
                    context.add_init_script("localStorage.setItem('unogo.token',"+json.dumps(user["token"])+");localStorage.setItem('unogo.table',"+json.dumps(code)+");")
                    page = context.new_page()
                    errors=[]
                    page.on("pageerror", lambda e:errors.append(str(e)))
                    def reply(route):
                        current['serverNow']=int(time.time()*1000)
                        phase=current['game']['phase']
                        if isinstance(phase,dict) and phase.get('TouchFive',{}).get('challenge')==1:
                            # Freeze this mocked layout case in the countdown state.
                            phase['TouchFive']['starts_at']=current['serverNow']+1200
                        route.fulfill(json=current)
                    page.route(f"**/api/lobbies/{code}", reply)
                    page.goto(f"http://127.0.0.1:{host.uno}", wait_until="domcontentloaded")
                    expect(page.locator(".hand-card")).to_have_count(40)
                    page.evaluate("""([top,bottom])=>{
                        document.documentElement.style.setProperty('--uno-safe-top',top+'px');
                        document.documentElement.style.setProperty('--uno-safe-bottom',bottom+'px');
                    }""", [top,bottom])
                    original_top=copy.deepcopy(current['game']['top'])
                    for phase,label in [({"Swap":{"player":0}},"swap"),({"TouchFive":{"player":0,"challenge":1,"starts_at":int(time.time()*1000)+1200,"taps":[]}},"five-ready"),({"TouchFive":{"player":0,"challenge":2,"starts_at":0,"taps":[]}},"five-tap"),({"TouchFive":{"player":0,"challenge":3,"starts_at":0,"taps":[0]}},"five-ack"),("Turn","turn"),({"Drawn":{"card":current['game']['hand'][0]['id']}},"drawn"),({"Uno":{"player":0}},"uno"),({"Challenge":{"sender":1,"recipient":0}},"challenge"),("Turn","empty-pile")]:
                        current["game"]["phase"] = phase
                        current['game']['top']={'id':9,'color':'Red','value':{'Number':5}} if label.startswith('five-') else original_top
                        current["game"]["pile_empty"] = label=="empty-pile"
                        expect(page.locator('.board-status h1')).to_contain_text(
                            re.compile({'swap':'Swap hands','five-ready':'Everyone, touch the 5!','five-tap':'Everyone, touch the 5!','five-ack':'Everyone, touch the 5!','turn':'Your turn','drawn':'Play the new card','uno':'UNO window','challenge':'Accept \\+4','empty-pile':'Your turn'}[label]))
                        if label.startswith('five-'):expect(page.locator('.touch-target')).to_have_class(re.compile({'five-ready':'counting','five-tap':'ready','five-ack':'tapped'}[label]))
                        if label=='empty-pile':expect(page.locator('.pile-notice')).to_be_visible()
                        metrics = page.evaluate("""()=>({height:innerHeight,page:document.documentElement.scrollHeight,
                            width:innerWidth,pageWidth:document.documentElement.scrollWidth,
                            actions:document.querySelector('.table-emotes').getBoundingClientRect().bottom,
                            board:document.querySelector('.game-board').getBoundingClientRect().bottom})""")
                        assert metrics["page"] <= height+1, (name,label,metrics)
                        assert metrics["pageWidth"] <= width, (name,label,metrics,page.evaluate("""()=>[...document.querySelectorAll('body *')].filter(e=>e.getBoundingClientRect().right>innerWidth&&!e.closest('.hand-card,.opponent,.reaction-strip')).slice(0,8).map(e=>[e.className,e.getBoundingClientRect().right])"""))
                        assert metrics["actions"] <= height-bottom, (name,label,metrics)
                        assert metrics["board"] <= height-bottom, (name,label,metrics)
                        page.locator(".hand-card").last.scroll_into_view_if_needed()
                        assert page.evaluate("scrollY")==0, (name,label,"hand scrolled the page")
                        page.screenshot(path=str(shots/f"{name}-{label}.png"))
                    current["game"]["phase"]="Turn"
                    current["game"]["pile_empty"]=False
                    assert not errors,errors
                    context.close()
                    print(f"PASS: {name}: 10 seats, 40 cards, turn/drawn/UNO/challenge fit without page scrolling")
            finally:
                browser.close()
    finally:
        host.close()


if __name__ == "__main__":
    main()
