"""Isolated real multiplayer/browser checks for Friends rules and locale controls."""
import json
import sqlite3
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
import browser_flow as flow
from live_host import ROOT, api


def main():
    flow.PUBLIC=ROOT/'unogo/target/dx/uno-client/release/web/public'
    flow.BINARY=ROOT/'rust/target/release/domino-server.exe'
    host=flow.Host();shots=ROOT/'unogo/docs/screenshots/house-rules';shots.mkdir(parents=True,exist_ok=True)
    users=[api(host.uno,'/api/auth/guest',body={'prefix':name}) for name in ['Host','Friend','Third']]
    def room(spec,top=('Red',7),bots=False):
        code=api(host.uno,'/api/lobbies',users[0]['token'],{'maxPlayers':3})['id']
        for user in users[1:]:
            api(host.uno,f'/api/lobbies/{code}/add-bot' if bots else f'/api/lobbies/{code}/join',users[0]['token'] if bots else user['token'],{})
        api(host.uno,f'/api/lobbies/{code}/start',users[0]['token'],{})
        with sqlite3.connect(host.database) as db:
            raw=json.loads(db.execute('SELECT state_json FROM uno_tables WHERE id=?',(code,)).fetchone()[0]);g=raw['game']
            pool=g['draw']+[c for h in g['hands'] for c in h]+g['discard']
            def take(color,number):
                c=next(c for c in pool if c['color']==color and c['value']=={'Number':number});pool.remove(c);return c
            hands=[[take(*item) for item in hand] for hand in spec];up=take(*top)
            g.update(hands=hands,draw=pool,discard=[up],turn=0,color=top[0],phase='Turn',pending=None,sequences=[0,0,0],reaction_serial=0,last_touch=None)
            now=int(time.time()*1000);g.update(last_time=now,deadline=now+600000)
            db.execute('UPDATE uno_tables SET state_json=?,saved_at=?,bot_due=? WHERE id=?',(json.dumps(raw),now,now+600000,code))
        db.close();return code
    def view(code,seat=0):return api(host.uno,f'/api/lobbies/{code}',users[seat]['token'])
    def command(code,seat,action,expected=200,sequence=None):
        v=view(code,seat)
        return api(host.uno,f'/api/lobbies/{code}/commands',users[seat]['token'],{'matchId':v['matchId'],'sequence':sequence or v['acceptedSequence']+1,'action':action},expected=expected)
    contexts=[]
    def page_for(browser,code,seat=0,width=393,height=852):
        c=browser.new_context(viewport={'width':width,'height':height});contexts.append(c)
        c.add_init_script("localStorage.setItem('unogo.token',"+json.dumps(users[seat]['token'])+");localStorage.setItem('unogo.table',"+json.dumps(code)+");if(!localStorage.getItem('domino.language'))localStorage.setItem('domino.language','en');")
        p=c.new_page();p.goto(f'http://127.0.0.1:{host.uno}',wait_until='domcontentloaded');expect(p.locator('.hand-card').first).to_be_visible();return p
    def play(page,card):
        e=page.locator(f".hand-card[data-card-id='{card['id']}']");e.focus();e.press('Enter');e.press('Enter')
    try:
        with sync_playwright() as p:
            browser=p.chromium.launch(channel='msedge',headless=True)
            try:
                # Direct selection: reject unmatched pairs; replace different numbers;
                # then allow a matching anchor selected after a nonmatching card.
                direct=[[('Blue',8),('Green',8),('Red',8),('Yellow',8),('Red',2)],[('Blue',3),('Blue',4)],[('Green',1),('Green',2)]]
                code=room(direct,('Red',7));a=page_for(browser,code);b=page_for(browser,code,1)
                cards=view(code)['game']['hand']
                def click_card(index):
                    tile=a.locator(f".hand-card[data-card-id='{cards[index]['id']}']")
                    from card_gestures import point
                    x,y=point(a,tile);a.mouse.click(x,y)
                assert a.locator('.group-toggle').count()==0
                click_card(0);click_card(1)
                expect(a.locator('.hand-card.selected')).to_have_count(1)
                expect(a.locator(f".hand-card[data-card-id='{cards[1]['id']}']")).to_have_attribute('aria-pressed','true')
                assert a.locator('.play-group').count()==0
                invalid=[cards[0]['id'],cards[1]['id']]
                command(code,0,{'PlayNumbers':{'cards':invalid,'uno':True}},expected=409)
                click_card(4);expect(a.locator('.hand-card.selected')).to_have_count(1)
                click_card(0);click_card(2);click_card(1);click_card(3)
                expect(a.locator('.hand-card.selected')).to_have_count(4)
                expect(a.get_by_role('button',name='Play 4',exact=True)).to_be_enabled()
                # Removing the sole matching anchor collapses the illegal remainder.
                click_card(2);expect(a.locator('.hand-card.selected')).to_have_count(1)
                assert a.locator('.play-group').count()==0
                a.keyboard.press('Escape');expect(a.locator('.hand-card.selected')).to_have_count(0)
                click_card(0);click_card(2);click_card(1);click_card(3)
                a.screenshot(path=str(shots/'direct-selection.png'))
                a.get_by_role('button',name='Play 4',exact=True).click()
                # An unarmed four-card play leaving one opens the real UNO window.
                expect(a.get_by_role('button',name='Call UNO!',exact=True)).to_be_visible()
                a.get_by_role('button',name='Call UNO!',exact=True).click()
                assert view(code)['game']['color']=='Yellow'
                expect(b.locator('.active-color')).to_contain_text('Yellow')
                expect(a.locator('.hand-card')).to_have_count(1);a.close();b.close()
                print('PASS: direct selection without Group button; unmatched same-number rejection, different-number replacement, late matching anchor, any-color extension, anchor removal and final color',flush=True)
                spec=[[('Green',8),('Yellow',8),('Red',0),('Red',5),('Blue',2)],[('Blue',3),('Blue',4),('Yellow',2)],[('Green',1),('Green',2)]]
                code=room(spec,('Red',8));a=page_for(browser,code);b=page_for(browser,code,1)
                initial=view(code)['game']['hand']
                for card in [initial[1],initial[0]]:
                    e=a.locator(f".hand-card[data-card-id='{card['id']}']");e.focus();e.press('Enter')
                expect(a.locator('.card-order-badge')).to_have_count(2)
                a.get_by_role('button',name='Move earlier 2',exact=True).click()
                a.screenshot(path=str(shots/'ordered-group.png'))
                a.get_by_role('button',name='Play 2',exact=True).click()
                expect(a.locator('.hand-card')).to_have_count(3)
                assert view(code)['game']['color']=='Yellow' and view(code)['game']['counts']==[3,3,2]
                expect(b.locator('.active-color')).to_contain_text('Yellow')
                bad=view(code,1);ids=[c['id'] for c in bad['game']['hand'][:2]]
                command(code,1,{'PlayNumbers':{'cards':ids,'uno':True}},expected=409)
                assert view(code,1)['game']['hand']==bad['game']['hand']
                assert a.locator('.card-controls,.table-reactions').count()==0
                expect(a.locator('.reaction-button').first).to_be_visible()
                # Locale survives both remount and reload; center text stays inside its plane.
                a.close();b.close();code=room(spec,('Red',8));a=page_for(browser,code);b=page_for(browser,code,1)
                for lang in ['az','tr','ru','en']:
                    a.locator('.profile-button').click();a.locator('.account-sheet select').select_option(lang)
                    a.locator('.account-sheet .round-control').first.click();a.locator('.profile-button').click()
                    expect(a.locator('.account-sheet select')).to_have_value(lang)
                    a.locator('.account-sheet .round-control').first.click()
                    for width,height in [(393,852),(320,568),(852,393)]:
                        a.set_viewport_size({'width':width,'height':height})
                        a.screenshot(path=str(shots/f'locale-{lang}-{width}.png'))
                        assert a.evaluate('document.documentElement.scrollHeight<=innerHeight+1&&document.documentElement.scrollWidth<=innerWidth'),(lang,width,height,a.evaluate("()=>({h:document.documentElement.scrollHeight,w:document.documentElement.scrollWidth,out:[...document.querySelectorAll('body *')].filter(e=>e.getBoundingClientRect().right>innerWidth&&!e.closest('.hand-card,.opponent,.reaction-strip')).map(e=>[e.className,e.getBoundingClientRect().right]),items:[...document.querySelector('.game-board').children].map(e=>[e.className,e.getBoundingClientRect().height,e.getBoundingClientRect().bottom])})"))
                        a.screenshot(path=str(shots/f'locale-{lang}-{width}.png'))
                    a.reload(wait_until='domcontentloaded');a.locator('.profile-button').click()
                    expect(a.locator('.account-sheet select')).to_have_value(lang);a.locator('.account-sheet .round-control').first.click()
                a.close();b.close()
                # Reverse group order is also accepted and sets the other last color.
                code=room(spec,('Red',8));v=view(code);ids=[v['game']['hand'][1]['id'],v['game']['hand'][0]['id']]
                a=page_for(browser,code)
                for card_id in ids:
                    e=a.locator(f".hand-card[data-card-id='{card_id}']");e.focus();e.press('Enter')
                from card_gestures import point
                source=a.locator(f".hand-card[data-card-id='{ids[0]}']");x,y=point(a,source)
                pile=a.locator('.discard-pile').bounding_box()
                a.mouse.move(x,y);a.mouse.down();a.mouse.move(x,y-15,steps=3)
                expect(a.locator('.bundle-count')).to_have_text('×2')
                a.mouse.move(pile['x']+pile['width']/2,pile['y']+pile['height']/2,steps=12)
                a.mouse.up();expect(a.locator('.hand-card')).to_have_count(3)
                assert view(code)['game']['color']=='Green';a.close()
                # Grouping down to one must expose UNO arming instead of silently declaring it.
                short=[[('Green',8),('Yellow',8),('Red',1)],spec[1],spec[2]]
                code=room(short,('Red',8));a=page_for(browser,code);cards=view(code)['game']['hand']
                for card in cards[:2]:
                    e=a.locator(f".hand-card[data-card-id='{card['id']}']");e.focus();e.press('Enter')
                expect(a.locator('.uno-arm')).to_be_visible();expect(a.locator('.uno-arm')).to_have_attribute('aria-pressed','false')
                a.locator('.uno-arm').click();a.get_by_role('button',name='Play 2',exact=True).click()
                expect(a.locator('.hand-card')).to_have_count(1);assert view(code)['game']['phase']=='Turn';a.close()
                print('PASS: chosen group order, earlier reordering, both final colors, atomic rejection, visible emotes, removed menu, four-locale remount/reload and responsive text',flush=True)
                zero_spec=[[('Red',0),('Red',1),('Blue',1)],[('Blue',3),('Blue',4),('Yellow',2)],[('Green',1),('Green',2)]]
                for swap in [False,True]:
                    code=room(zero_spec);a=page_for(browser,code);b=page_for(browser,code,1)
                    va,vb=view(code),view(code,1);play(a,va['game']['hand'][0])
                    expect(a.get_by_role('button',name='Keep my cards',exact=True)).to_be_visible()
                    if swap:
                        a.get_by_role('button',name='Swap hands',exact=True).click()
                        assert a.evaluate("[...document.querySelectorAll('.card-motion-layer')].every(e=>getComputedStyle(e).visibility==='hidden')"),'Card animation covers the swap decision'
                        a.screenshot(path=str(shots/'optional-swap.png'))
                        with a.expect_response(lambda r:r.url.endswith('/commands')):
                            a.locator('.swap-targets button').first.click()
                        expect(a.locator('.hand-card')).to_have_count(3);expect(b.locator('.hand-card')).to_have_count(2)
                        assert view(code)['game']['hand']==vb['game']['hand']
                        assert view(code,1)['game']['hand']==va['game']['hand'][1:]
                    else:
                        with a.expect_response(lambda r:r.url.endswith('/commands')):
                            a.get_by_role('button',name='Keep my cards',exact=True).click()
                        assert view(code)['game']['hand']==va['game']['hand'][1:]
                    assert view(code)['game']['turn']==1;a.close();b.close()
                print('PASS: optional 0 keep/swap UI and exact private hand/count synchronization',flush=True)
                five_spec=[[('Red',5),('Red',1),('Blue',2)],[('Blue',3),('Blue',4),('Yellow',2)],[('Green',1),('Green',2)]]
                code=room(five_spec);a=page_for(browser,code);b=page_for(browser,code,1)
                play(a,view(code)['game']['hand'][0]);button=b.get_by_role('button',name='Tap the 5',exact=True)
                expect(button).to_be_enabled(timeout=4000);button.click()
                expect(button).to_be_disabled();expect(b.locator('.touch-stamp')).to_contain_text('1')
                b.screenshot(path=str(shots/'five-tap-ack.png'))
                phase=view(code)['game']['phase']['TouchFive'];challenge=phase['challenge'];assert phase['taps']==[1]
                command(code,1,{'TouchFive':{'challenge':challenge}},expected=409)
                assert view(code)['game']['phase']['TouchFive']['taps']==[1]
                a.get_by_role('button',name='Tap the 5',exact=True).click()
                command(code,2,{'TouchFive':{'challenge':challenge}})
                result=view(code)['game'];assert result['last_touch']['order']==[1,0,2] and result['last_touch']['penalty']==2
                assert result['counts']==[2,3,3]
                expect(a.locator('.tap-outcome')).to_contain_text('drew one card');a.screenshot(path=str(shots/'five-result.png'));a.close();b.close()
                # Real concurrent HTTP arrivals are serialized, with one penalty and no duplicate seats.
                code=room(five_spec);command(code,0,{'Play':{'card':view(code)['game']['hand'][0]['id'],'color':None,'uno':True}})
                phase=view(code)['game']['phase']['TouchFive'];time.sleep(max(0,(phase['starts_at']-int(time.time()*1000))/1000)+.03)
                with ThreadPoolExecutor(max_workers=3) as pool:list(pool.map(lambda seat:command(code,seat,{'TouchFive':{'challenge':phase['challenge']}}),range(3)))
                final=view(code)['game'];r=final['last_touch'];assert sorted(r['order'])==[0,1,2] and r['penalty']==r['order'][-1]
                assert sum(final['counts'])==8
                # Partial tapping survives restart; disconnected players cannot freeze the room.
                code=room(five_spec);command(code,0,{'Play':{'card':view(code)['game']['hand'][0]['id'],'color':None,'uno':True}})
                phase=view(code)['game']['phase']['TouchFive'];time.sleep(max(0,(phase['starts_at']-int(time.time()*1000))/1000)+.03)
                command(code,0,{'TouchFive':{'challenge':phase['challenge']}});host.stop();host.start()
                assert view(code)['game']['phase']['TouchFive']['taps']==[0]
                for _ in range(150):
                    g=view(code)['game']
                    if g['last_touch']:break
                    time.sleep(.05)
                else:raise AssertionError('Unresponsive seats froze the game')
                assert g['last_touch']['penalty'] in [1,2] and g['phase']=='Turn'
                # Bot taps are scheduled independently of the current turn and human commands.
                code=room(five_spec,bots=True);command(code,0,{'Play':{'card':view(code)['game']['hand'][0]['id'],'color':None,'uno':True}})
                phase=view(code)['game']['phase']['TouchFive'];time.sleep(max(0,(phase['starts_at']-int(time.time()*1000))/1000)+.03)
                command(code,0,{'TouchFive':{'challenge':phase['challenge']}})
                for _ in range(70):
                    g=view(code)['game']
                    if g['last_touch']:break
                    time.sleep(.05)
                assert g['last_touch'] and sorted(g['last_touch']['order'])==[0,1,2]
                print('PASS: shared 5 countdown, tap acknowledgements/order, duplicates, one penalty, concurrent arrivals, restart/timeout and mixed bots',flush=True)
            finally:
                for c in contexts:c.close()
                browser.close()
    finally:host.close()


if __name__=='__main__':main()
