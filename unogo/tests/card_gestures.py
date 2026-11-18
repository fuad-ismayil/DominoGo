"""Real pointer/touch gestures, server-authoritative plays, and rollback in an isolated host."""
import json
import sqlite3
import time
from playwright.sync_api import sync_playwright, expect
import browser_flow as flow
from live_host import ROOT, api


def point(page, locator):
    result=locator.evaluate("""e=>{const r=e.getBoundingClientRect();
        for(let y=r.top+18;y<r.bottom-12;y+=8)for(let x=r.left+8;x<r.right-8;x+=8)
            if(document.elementFromPoint(x,y)?.closest('[data-card-id]')===e)return [x,y];
        return null;}""")
    assert result, "Card has no touchable exposed area"
    return result


def main():
    flow.PUBLIC=ROOT/'unogo/target/dx/uno-client/release/web/public'
    flow.BINARY=ROOT/'rust/target/release/domino-server.exe'
    host=flow.Host();shots=ROOT/'unogo/docs/screenshots/card-gestures';shots.mkdir(parents=True,exist_ok=True)
    try:
        user=api(host.uno,'/api/auth/guest',body={'prefix':'Gesture'})
        code=api(host.uno,'/api/lobbies',user['token'],{'maxPlayers':2})['id']
        api(host.uno,f'/api/lobbies/{code}/add-bot',user['token'],{})
        api(host.uno,f'/api/lobbies/{code}/start',user['token'],{})
        with sqlite3.connect(host.database) as db:
            raw=json.loads(db.execute('SELECT state_json FROM uno_tables WHERE id=?',(code,)).fetchone()[0]);g=raw['game']
            pool=g['draw']+[c for h in g['hands'] for c in h]+g['discard']
            def take(predicate):
                card=next(c for c in pool if predicate(c));pool.remove(card);return card
            top=take(lambda c:c['color']=='Red' and c['value']=={'Number':3})
            legal=take(lambda c:c['color']=='Red' and c['value']=={'Number':6})
            illegal=take(lambda c:c['color']=='Blue' and c['value']=={'Number':2})
            wild=take(lambda c:c['value']=='Wild')
            # Predictable numbered replies keep penalties out of this gesture-only fixture.
            bot=[take(lambda c:c['color']==color and isinstance(c['value'],dict) and c['value'].get('Number') not in [0,5]) for color in ['Red','Blue','Red','Blue','Red','Blue','Red']]
            g['hands']=[[legal,illegal,wild]+pool[:37],bot];g['draw']=pool[37:];g['discard']=[top]
            now=int(time.time()*1000);g.update(turn=0,color='Red',phase='Turn',deadline=now+600000,last_time=now,pending=None,rules_version='classic-108-single-v1')
            assert sum(map(len,g['hands']))+len(g['draw'])+len(g['discard'])==108
            db.execute('UPDATE uno_tables SET state_json=?,saved_at=?,bot_due=? WHERE id=?',(json.dumps(raw),now,now+600000,code))
        db.close()
        with sync_playwright() as p:
            browser=p.chromium.launch(channel='msedge',headless=True)
            try:
                context=browser.new_context(viewport={'width':393,'height':852},has_touch=True,is_mobile=True)
                context.add_init_script("localStorage.setItem('unogo.token',"+json.dumps(user['token'])+");localStorage.setItem('unogo.table',"+json.dumps(code)+");localStorage.setItem('domino.language','en');")
                page=context.new_page();errors=[];commands=[]
                page.on('pageerror',lambda e:errors.append(str(e)))
                page.on('request',lambda r: commands.append(r.post_data_json) if r.url.endswith('/commands') else None)
                page.goto(f'http://127.0.0.1:{host.uno}',wait_until='domcontentloaded')
                expect(page.locator('.hand-card')).to_have_count(40);page.wait_for_timeout(900)
                card=page.locator(f".hand-card[data-card-id='{legal['id']}']")
                bad=page.locator(f".hand-card[data-card-id='{illegal['id']}']")
                wild_card=page.locator(f".hand-card[data-card-id='{wild['id']}']")
                pile=page.locator('.discard-pile').bounding_box();target=(pile['x']+pile['width']/2,pile['y']+pile['height']/2)
                def drag(locator,end):
                    x,y=point(page,locator);page.mouse.move(x,y);page.mouse.down();page.mouse.move(x,y-15,steps=3)
                    expect(page.locator('.card-flight.is-held')).to_be_visible()
                    page.mouse.move(*end,steps=12)
                # Missed and illegal drops never submit a command.
                drag(card,(12,450));page.mouse.up();page.wait_for_timeout(350);assert not commands
                drag(bad,target);expect(page.locator('.game-board.drop-ready')).to_have_count(0);page.mouse.up();page.wait_for_timeout(350);assert not commands
                # Cancellation releases capture and restores the source card.
                drag(card,target);card.dispatch_event('pointercancel',{'pointerId':1,'isPrimary':True});page.mouse.up();page.wait_for_timeout(350)
                assert not commands and page.locator('.card-flight').count()==0
                # Native touch swipe pans a crowded hand without playing a card.
                client=context.new_cdp_session(page);x,y=point(page,card)
                client.send('Input.dispatchTouchEvent',{'type':'touchStart','touchPoints':[{'x':x,'y':y}]})
                for dx in [10,30,60,100]:client.send('Input.dispatchTouchEvent',{'type':'touchMove','touchPoints':[{'x':x-dx,'y':y}]})
                client.send('Input.dispatchTouchEvent',{'type':'touchEnd','touchPoints':[]})
                assert page.locator('.hand-track').evaluate('e=>e.scrollLeft')>0 and not commands
                page.locator('.hand-track').evaluate('e=>e.scrollLeft=0');page.wait_for_timeout(250)
                # Wild drop opens color selection without committing; cancel preserves the hand.
                drag(wild_card,target);page.mouse.up()
                expect(page.get_by_role('dialog',name='Choose a color')).to_be_visible();assert not commands
                page.get_by_role('button',name='Cancel',exact=True).click();expect(page.locator('.hand-card')).to_have_count(40)
                # Single tap lifts; keyboard controls remain available behind their disclosure.
                x,y=point(page,card);page.mouse.click(x,y);expect(card).to_have_class(__import__('re').compile('selected'))
                assert page.locator('.card-controls').count()==0
                assert page.locator('.table-emotes').is_visible()
                page.keyboard.press('Escape')
                # A rejected network request does not remove a card or advance its sequence.
                sequence=api(host.uno,f'/api/lobbies/{code}',user['token'])['acceptedSequence']
                page.route('**/commands',lambda route:route.fulfill(status=503,json={'error':'Test request failure'}))
                drag(card,target);page.screenshot(path=str(shots/'held-over-table.png'));page.mouse.up()
                expect(page.locator('.notice')).to_be_visible();expect(page.locator('.hand-card')).to_have_count(40)
                assert api(host.uno,f'/api/lobbies/{code}',user['token'])['acceptedSequence']==sequence
                page.unroute('**/commands');page.locator('.notice button').click();page.wait_for_timeout(500)
                # A real touch drag commits exactly once through the existing command API.
                before=len(commands);x,y=point(page,card)
                client.send('Input.dispatchTouchEvent',{'type':'touchStart','touchPoints':[{'x':x,'y':y}]});page.wait_for_timeout(210)
                for i in range(1,13):client.send('Input.dispatchTouchEvent',{'type':'touchMove','touchPoints':[{'x':x+(target[0]-x)*i/12,'y':y+(target[1]-y)*i/12}]})
                client.send('Input.dispatchTouchEvent',{'type':'touchEnd','touchPoints':[]})
                expect(page.locator('.hand-card')).to_have_count(39)
                assert len(commands)==before+1
                assert api(host.uno,f'/api/lobbies/{code}',user['token'])['acceptedSequence']==sequence+1
                # Complete a Wild drag, then verify the chosen second-tap interaction.
                expect(page.locator('.board-status h1')).to_contain_text('Your turn',timeout=10000)
                wild_card.scroll_into_view_if_needed();drag(wild_card,target);page.mouse.up()
                expect(page.get_by_role('dialog',name='Choose a color')).to_be_visible()
                before=len(commands);page.get_by_role('button',name='Blue',exact=True).click()
                expect(page.locator('.hand-card')).to_have_count(38);assert len(commands)==before+1
                expect(page.locator('.board-status h1')).to_contain_text('Your turn',timeout=10000)
                view=api(host.uno,f'/api/lobbies/{code}',user['token'])['game']
                pick=next(c for c in view['hand'] if c['id'] in view['playable'] and c['color'] is not None)
                tap_card=page.locator(f".hand-card[data-card-id='{pick['id']}']");tap_card.scroll_into_view_if_needed()
                before=len(commands)
                def tap(locator):
                    x,y=point(page,locator)
                    client.send('Input.dispatchTouchEvent',{'type':'touchStart','touchPoints':[{'x':x,'y':y}]})
                    client.send('Input.dispatchTouchEvent',{'type':'touchEnd','touchPoints':[]})
                tap(tap_card);expect(tap_card).to_have_class(__import__('re').compile('selected'));assert len(commands)==before
                page.wait_for_timeout(180);tap(tap_card);expect(page.locator('.hand-card')).to_have_count(37)
                assert len(commands)==before+1
                page.emulate_media(reduced_motion='reduce')
                assert page.locator('.hand-card').first.evaluate('e=>getComputedStyle(e).animationName')=='none'
                assert page.evaluate('scrollY')==0 and page.evaluate('document.documentElement.scrollWidth<=innerWidth')
                assert not errors,errors
                page.screenshot(path=str(shots/'after-touch-play.png'))
                context.close()
                print('PASS: native touch drag commits once; horizontal swipe, cancellation, illegal/missed drops, Wild cancel/commit, second-tap play, fallback controls, reduced motion, server failure recovery')
            finally:browser.close()
    finally:host.close()


if __name__=='__main__':main()
