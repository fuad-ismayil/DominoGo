"""Card/table geometry, readable UNO toggles and everyone's animated reactions."""
import json
import sqlite3
import time
from playwright.sync_api import sync_playwright, expect
import browser_flow as flow
from live_host import ROOT, api


def main():
    flow.PUBLIC=ROOT/'unogo/target/dx/uno-client/release/web/public'
    flow.BINARY=ROOT/'rust/target/release/domino-server.exe'
    host=flow.Host();shots=ROOT/'unogo/docs/screenshots/visual-feedback';shots.mkdir(parents=True,exist_ok=True)
    try:
        users=[api(host.uno,'/api/auth/guest',body={'prefix':name}) for name in ['Host','Friend']]
        code=api(host.uno,'/api/lobbies',users[0]['token'],{'maxPlayers':2})['id']
        api(host.uno,f'/api/lobbies/{code}/join',users[1]['token'],{})
        api(host.uno,f'/api/lobbies/{code}/start',users[0]['token'],{})
        with sqlite3.connect(host.database) as db:
            raw=json.loads(db.execute('SELECT state_json FROM uno_tables WHERE id=?',(code,)).fetchone()[0]);g=raw['game']
            pool=g['draw']+[c for h in g['hands'] for c in h]+g['discard']
            def take(color,n):
                c=next(c for c in pool if c['color']==color and c['value']=={'Number':n});pool.remove(c);return c
            top=take('Red',7);hand=[take('Red',2),take('Blue',3)]
            g.update(hands=[hand,pool[:7]],draw=pool[7:],discard=[top],turn=0,color='Red',phase='Turn',pending=None)
            now=int(time.time()*1000);g.update(last_time=now,deadline=now+600000)
            db.execute('UPDATE uno_tables SET state_json=?,saved_at=? WHERE id=?',(json.dumps(raw),now,code))
        db.close()
        with sync_playwright() as p:
            browser=p.chromium.launch(channel='msedge',headless=True);contexts=[];pages=[];errors=[]
            try:
                for u in users:
                    context=browser.new_context(viewport={'width':393,'height':852});contexts.append(context)
                    context.add_init_script("localStorage.setItem('unogo.token',"+json.dumps(u['token'])+");localStorage.setItem('unogo.table',"+json.dumps(code)+");localStorage.setItem('domino.language','en');")
                    page=context.new_page();page.on('pageerror',lambda e:errors.append(str(e)))
                    page.goto(f'http://127.0.0.1:{host.uno}',wait_until='domcontentloaded');expect(page.locator('.hand-card').first).to_be_visible();pages.append(page)
                a,b=pages
                for width,height,name in [(393,852,'phone'),(320,568,'small-phone'),(852,393,'landscape'),(1440,960,'desktop')]:
                    a.set_viewport_size({'width':width,'height':height});a.wait_for_timeout(150)
                    assert a.locator('.card-color').count()==0
                    m=a.evaluate("""()=>{const r=s=>document.querySelector(s).getBoundingClientRect();const t=r('.card-table'),d=r('.discard-pile'),deck=r('.draw-pile'),c=r('.active-color');return {tableX:t.x+t.width/2,cardX:d.x+d.width/2,tableTop:t.top,colorBottom:c.bottom,deckRight:deck.right,cardLeft:d.left,deckWidth:deck.width,cardWidth:d.width,deckTop:deck.top,cardTop:d.top,pageW:document.documentElement.scrollWidth,pageH:document.documentElement.scrollHeight};}""")
                    assert abs(m['tableX']-m['cardX'])<2,(name,m)
                    assert m['deckRight']<m['cardLeft']-3 and m['deckWidth']<m['cardWidth'],(name,m)
                    assert m['deckTop']<m['cardTop']+4,(name,m)
                    if height>600:assert m['tableTop']>=m['colorBottom']+6,(name,m)
                    assert m['pageW']<=width and m['pageH']<=height+1,(name,m)
                    uno=a.locator('.uno-arm');expect(uno).to_have_text('UNO!')
                    for armed in [True,False,True,False]:
                        uno.click();uno.hover();expect(uno).to_have_attribute('aria-pressed',str(armed).lower());expect(uno).to_have_text('UNO!')
                        expect(uno).to_have_css('background-color','rgb(255, 220, 135)' if armed else 'rgb(204, 233, 255)')
                        colors=uno.evaluate("e=>({bg:getComputedStyle(e).backgroundColor,fg:getComputedStyle(e).color,svg:e.querySelectorAll('svg').length})")
                        assert colors['bg']==('rgb(255, 220, 135)' if armed else 'rgb(204, 233, 255)'),(name,armed,colors)
                        assert colors['fg']!=colors['bg'] and colors['svg']==int(armed),(name,armed,colors)
                    a.screenshot(path=str(shots/f'{name}-table.png'))
                a.set_viewport_size({'width':393,'height':852})
                for page in pages:
                    with page.expect_response(lambda r:r.url.endswith('/reactions') and r.request.method=='POST') as reply:
                        page.locator('.reaction-button').first.click()
                    assert reply.value.status==200
                for page in pages:
                    expect(page.locator('.own-reaction')).to_have_count(1)
                    expect(page.locator('.reaction-bubble:not(.own-reaction)')).to_have_count(1)
                    expect(page.locator('.own-reaction')).to_be_visible()
                    expect(page.locator('.reaction-bubble:not(.own-reaction)')).to_be_visible()
                    assert page.locator('.own-reaction').evaluate("e=>getComputedStyle(e).animationName")=='uno-reaction-rise'
                own=a.locator('.own-reaction');identity=own.get_attribute('data-reaction-id');elapsed=own.evaluate('e=>e.getAnimations()[0].currentTime')
                a.wait_for_timeout(450)
                assert own.get_attribute('data-reaction-id')==identity
                assert own.evaluate('e=>e.getAnimations()[0].currentTime')>elapsed+300,'Polling restarted the animation'
                a.screenshot(path=str(shots/'own-and-opponent-emotes.png'))
                expect(own).to_have_count(0,timeout=5000)
                # The same emoji is a new event; reduced motion shows a static emote.
                a.emulate_media(reduced_motion='reduce')
                with a.expect_response(lambda r:r.url.endswith('/reactions') and r.request.method=='POST'):
                    a.locator('.reaction-button').first.click()
                expect(own).to_have_count(1);expect(own).to_be_visible()
                assert own.get_attribute('data-reaction-id')!=identity
                assert own.evaluate("e=>getComputedStyle(e).animationName")=='none'
                assert not errors,errors
                print('PASS: no card labels; centered discard, smaller upper-left deck, clear status; UNO on/off/hover/checkmark at four sizes; own and remote animated emotes, no poll restart, replay and reduced motion',flush=True)
            finally:
                for context in contexts:context.close()
                browser.close()
    finally:host.close()


if __name__=='__main__':main()
