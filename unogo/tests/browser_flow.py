"""Real-browser invite-to-results checks against an isolated shared host.

Requires: pip install playwright; installed Microsoft Edge (no browser download).
Build with unogo/build.ps1 and cargo build --manifest-path rust/Cargo.toml -p domino-server.
Run from repository root: python unogo/tests/browser_flow.py
"""
import json
import re
import hashlib
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import time
from playwright.sync_api import sync_playwright, expect
from live_host import ROOT, BINARY, api, free_port

SHOTS = ROOT / "unogo/docs/screenshots"
PUBLIC = ROOT / "unogo/target/dx/uno-client/debug/web/public"
CAPTURES = []


class Host:
    def __init__(self):
        self.temp = tempfile.TemporaryDirectory(prefix="unogo-browser-")
        self.directory = Path(self.temp.name)
        self.database = self.directory / "browser.sqlite"
        self.domino, self.uno = free_port(), free_port()
        while self.uno == self.domino:
            self.uno = free_port()
        self.process = None
        self.start()

    def start(self):
        env = dict(os.environ, DOMINO_PORT=str(self.domino), UNOGO_PORT=str(self.uno),
                   DOMINO_DB_PATH=str(self.database), UNOGO_STATIC_DIR=str(PUBLIC))
        env.pop("DOMINO_STATIC_DIR", None)
        log = (self.directory / "host.log").open("ab")
        self.process = subprocess.Popen([str(BINARY)], cwd=self.directory, env=env,
                                        stdout=log, stderr=log,
                                        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        log.close()
        for _ in range(150):
            if self.process.poll() is not None:
                raise AssertionError((self.directory / "host.log").read_text())
            try:
                api(self.uno, "/api/health")
                return
            except OSError:
                time.sleep(.1)
        raise AssertionError("Host did not start")

    def stop(self):
        if self.process and self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=10)

    def close(self):
        self.stop()
        self.temp.cleanup()


def card_control(page, name):
    if name in ["Play card", "Play drawn card"]:
        return page.locator(".hand-card.selected").first
    if name in ["Draw", "Keep card"]:
        return page.locator(".draw-pile")
    return page.get_by_role("button", name=name, exact=True)


def token(page):
    return page.evaluate("localStorage.getItem('unogo.token')")


def screenshot(page, name):
    page.wait_for_timeout(350)
    page.screenshot(path=str(SHOTS / name), full_page=True, animations="disabled")
    assert page.evaluate("document.documentElement.scrollWidth <= innerWidth"), name
    CAPTURES.append({"path":name,"viewport":page.viewport_size,"fullPage":True,"required":True,"capturedAt":time.time()})


def large_table_and_languages(host, page, base):
    page.get_by_role("button", name="Back home", exact=True).click()
    expect(page.get_by_role("button", name="Create a table", exact=False)).to_be_enabled()
    page.get_by_role("button", name="Create a table", exact=False).click()
    page.get_by_role("button", name="10", exact=True).click()
    page.get_by_role("button", name="Create table", exact=True).click()
    page.locator("#invite-link").wait_for()
    code = page.locator("#invite-link").input_value().split("join=")[1]
    for _ in range(9):
        with page.expect_response(lambda r: r.url.endswith("/add-bot")):
            page.get_by_role("button", name="Add a bot", exact=True).click()
        expect(page.locator(".brand-button")).to_be_enabled()
    page.get_by_role("button", name="Start match", exact=True).click()
    page.locator(".game-board").wait_for()
    # A valid, conserved 40-card hand fixture stresses layout without adding
    # privileged routes or test behavior to the application.
    with sqlite3.connect(host.database) as db:
        raw = json.loads(db.execute("SELECT state_json FROM uno_tables WHERE id=?", (code,)).fetchone()[0])
        game = raw["game"]
        pool = game["draw"] + [c for hand in game["hands"] for c in hand] + game["discard"][:-1]
        assert len(pool) == 107 and len({c["id"] for c in pool}) == 107
        pool.sort(key=lambda c: c["color"] is not None)
        game["hands"] = [pool[:40]] + [pool[40 + i*7:47 + i*7] for i in range(9)]
        game["draw"] = pool[103:]
        game["discard"] = game["discard"][-1:]
        timestamp = int(time.time() * 1000)
        game.update(turn=0, phase="Turn", deadline=timestamp+30000, pending=None,
                    paused_remaining=None, last_time=timestamp, stalled=0)
        raw["pause_votes"] = []
        db.execute("UPDATE uno_tables SET state_json=?,saved_at=?,bot_due=? WHERE id=?",
                   (json.dumps(raw), timestamp, timestamp+30000, code))
    db.close()
    expect(page.locator(".hand-card")).to_have_count(40)
    wild=page.locator(".hand-card:has(.card-art.Wild)").first
    expect(wild).to_be_enabled()
    # A fanned hand exposes each card's edge; keyboard selection avoids
    # incorrectly targeting its covered center in this large-hand fixture.
    wild.focus()
    wild.press("Enter")
    before=api(host.uno,f"/api/lobbies/{code}",token(page))
    card_control(page,"Play card").click()
    page.set_viewport_size({"width":390,"height":844})
    screenshot(page,"color-picker-phone.png")
    page.get_by_role("dialog",name="Choose a color").get_by_role("button",name="Cancel",exact=True).click()
    after=api(host.uno,f"/api/lobbies/{code}",token(page))
    assert before["acceptedSequence"]==after["acceptedSequence"]
    assert before["game"]["hand"]==after["game"]["hand"]
    assert before["game"]["deadline"]==after["game"]["deadline"]
    for width, height, name in [(1440,960,"game-ten-40-desktop.png"),(390,844,"game-ten-40-phone.png"),(320,740,"game-ten-40-small-phone.png")]:
        page.set_viewport_size({"width":width,"height":height})
        screenshot(page,name)
        assert page.locator(".hand-track").evaluate("e=>e.scrollWidth>e.clientWidth")
        assert page.locator(".hand-card").first.bounding_box()["width"] >= 76
        assert page.locator(".club-navigation").count() == 0
    page.locator(".hand-card").last.scroll_into_view_if_needed()
    assert page.locator(".hand-track").evaluate("e=>e.scrollLeft>0")
    page.set_viewport_size({"width":390,"height":844})
    page.locator(".profile-button").click()
    page.locator(".account-sheet").wait_for()
    for language in ["az","tr","ru","en"]:
        page.locator(".account-sheet select").select_option(language)
        assert page.locator("html").get_attribute("lang") == language
        screenshot(page,f"account-phone-{language}.png")
        page.keyboard.press("Shift+Tab")
        assert page.evaluate("!!document.activeElement.closest('[role=dialog]')")
    page.locator(".account-sheet .round-control").first.click()
    expect(page.locator(".profile-button")).to_be_focused()
    print("PASS: ten-seat layout, conserved 40-card scrolling hand, 320/390/1440px, four languages and account focus trap")


def registered_friend_invitation(host, browser, base, errors):
    password = "browser-test-password"
    other = api(host.domino,"/api/auth/register",body={"username":"FriendSaved","password":password})
    context = browser.new_context(viewport={"width":1280,"height":900}, reduced_motion="reduce")
    first, second = context.new_page(), context.new_page()
    for page in [first,second]:
        page.on("pageerror",lambda e:errors.append(str(e)))
    first.goto(base)
    first.get_by_role("button",name="Create an account",exact=True).click()
    first.locator("#username").fill("HostSaved")
    first.locator("#password").fill(password)
    first.get_by_role("button",name="Create account",exact=True).click()
    expect(first.get_by_role("button",name="Create a table",exact=False)).to_be_enabled()
    # Separate storage is required for the second identity.
    other_context=browser.new_context(viewport={"width":390,"height":844},reduced_motion="reduce")
    second.close();second=other_context.new_page();second.on("pageerror",lambda e:errors.append(str(e)))
    second.goto(base)
    second.get_by_role("button",name="Sign in",exact=True).click()
    second.locator("#username").fill("FriendSaved")
    second.locator("#password").fill(password)
    second.get_by_role("button",name="Sign in",exact=True).click()
    expect(second.get_by_role("button",name="Create a table",exact=False)).to_be_enabled()
    first.locator(".club-navigation").get_by_role("button",name="Friends",exact=True).click()
    first.locator("#friend-name").fill("FriendSaved")
    first.get_by_role("button",name="Send request",exact=True).click()
    second.locator(".club-navigation").get_by_role("button",name="Friends",exact=True).click()
    second.get_by_role("button",name="Accept",exact=True).click()
    expect(second.locator(".friend-row")).to_have_count(1)
    expect(first.locator(".friend-row")).to_have_count(1,timeout=15000)
    assert len(api(host.domino,"/api/friends",other["token"])["friends"])==1
    first.locator(".club-navigation").get_by_role("button",name="Home",exact=True).click()
    first.get_by_role("button",name="Create a table",exact=False).click()
    first.get_by_role("button",name="Create table",exact=True).click()
    first.get_by_role("button",name="Invite a friend",exact=True).click()
    first.locator(".invite-list").get_by_role("button",name="Invite",exact=True).click()
    second.locator(".invite-toast").get_by_role("button",name="Join table",exact=True).click()
    expect(first.locator(".seat-row:not(.empty-seat)")).to_have_count(2)
    expect(second.get_by_role("button",name="Leave table",exact=True)).to_be_visible()
    assert not api(host.uno,"/api/invitations",token(second))
    first.locator(".profile-button").click()
    first.get_by_text("Change password",exact=True).click()
    first.locator("#new-password").fill("updated-browser-password")
    first.route("**/api/auth/reset-password",lambda route:route.fulfill(status=503,content_type="application/json",body=json.dumps({"error":"Temporary test failure"})))
    first.get_by_role("button",name="Update password",exact=True).click()
    expect(first.locator(".account-sheet [role=alert]")).to_contain_text("Temporary test failure")
    expect(first.get_by_role("button",name="Update password",exact=True)).to_be_enabled()
    first.unroute("**/api/auth/reset-password")
    with first.expect_response(lambda r:r.url.endswith("/api/auth/reset-password")) as response:
        first.get_by_role("button",name="Update password",exact=True).click()
    assert response.value.status==200
    expect(first.locator(".account-success")).to_contain_text("Password updated")
    expect(first.locator(".account-sheet [role=alert]")).to_have_count(0)
    expect(first.get_by_role("button",name="Update password",exact=True)).to_be_enabled()
    screenshot(first,"password-account-desktop.png")
    api(host.domino,"/api/auth/login",body={"username":"HostSaved","password":password},expected=401)
    api(host.domino,"/api/auth/login",body={"username":"HostSaved","password":"updated-browser-password"})
    print("PASS: registered account password request failure recovery, successful update, and new shared credentials")
    print("PASS: UI registration/login, shared friendship request/accept, live UNO friend invitation and waiting-room join")
    context.close();other_context.close()


def main():
    SHOTS.mkdir(parents=True, exist_ok=True)
    host = Host()
    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(channel="msedge", headless=True)
            desktop = browser.new_context(viewport={"width": 1440, "height": 960}, reduced_motion="reduce")
            mobile = browser.new_context(viewport={"width": 390, "height": 844}, is_mobile=True, has_touch=True, reduced_motion="reduce")
            a, b = desktop.new_page(), mobile.new_page()
            errors = []
            for page in [a, b]:
                page.on("pageerror", lambda e: errors.append(str(e)))
                page.set_default_timeout(15000)
            base = f"http://127.0.0.1:{host.uno}"
            a.goto(base)
            a.get_by_role("button", name="Play as guest", exact=True).wait_for()
            screenshot(a, "entry-desktop.png")
            a.locator("#guest-name").fill("CardHost")
            a.get_by_role("button", name="Play as guest", exact=True).click()
            expect(a.get_by_role("button", name="Create a table", exact=False)).to_be_enabled()
            screenshot(a, "home-desktop.png")
            a.get_by_role("button", name="Create a table", exact=False).click()
            screenshot(a, "setup-desktop.png")
            a.get_by_role("button", name="Create table", exact=True).click()
            a.locator("#invite-link").wait_for()
            invite = a.locator("#invite-link").input_value()
            code = invite.split("join=")[1]
            assert invite.startswith(base), "Local links must retain the local origin"
            b.goto(invite)
            b.get_by_role("button", name="Leave table", exact=True).wait_for()
            expect(a.locator(".seat-row:not(.empty-seat)")).to_have_count(2)
            screenshot(a, "lobby-desktop.png")
            screenshot(b, "lobby-phone.png")
            # Reload rejoins exactly one existing seat and never starts the match.
            b.reload()
            b.get_by_role("button", name="Leave table", exact=True).wait_for()
            state = api(host.uno, f"/api/lobbies/{code}", token(b))
            assert len(state["players"]) == 2 and state["status"] == "waiting"
            a.get_by_role("button", name="Start match", exact=True).click()
            a.locator(".game-board").wait_for()
            b.locator(".game-board").wait_for()
            screenshot(a, "game-desktop.png")
            screenshot(b, "game-phone.png")

            # A successful reaction is visible to the other human and counted once.
            a.locator(".reaction-button").first.wait_for()
            with a.expect_response(lambda r: r.url.endswith("/reactions") and r.request.method == "POST"):
                a.locator(".reaction-button").first.click()
            expect(b.locator(".reaction-bubble")).to_have_count(1)

            # Pause votes freeze timers; restarting the process preserves both hands.
            a.get_by_role("button", name="Pause", exact=True).click()
            b.get_by_role("button", name="Pause", exact=True).click()
            a.get_by_role("button", name="Continue game", exact=True).wait_for()
            before = api(host.uno, f"/api/lobbies/{code}", token(b))["game"]["hand"]
            host.stop()
            expect(a.locator(".connection-status")).to_contain_text("Reconnecting", timeout=15000)
            assert not a.get_by_role("button", name="Continue game", exact=True).is_enabled()
            host.start()
            expect(a.get_by_role("button", name="Continue game", exact=True)).to_be_enabled(timeout=15000)
            assert api(host.uno, f"/api/lobbies/{code}", token(b))["game"]["hand"] == before
            a.get_by_role("button", name="Continue game", exact=True).click()
            expect(a.get_by_role("dialog", name="Game paused")).to_have_count(0)
            # Restore a browser session in a different pending decision-free turn.
            b.reload()
            b.locator(".game-board").wait_for()

            canceled_wild = False
            for step in range(1500):
                state = api(host.uno, f"/api/lobbies/{code}", token(a))
                if state["status"] == "finished":
                    break
                phase = state["game"]["phase"]
                seat = phase["Uno"]["player"] if isinstance(phase, dict) and "Uno" in phase else state["game"]["turn"]
                if isinstance(phase,dict) and "TouchFive" in phase:
                    data=phase["TouchFive"]
                    if not state["game"]["touch_ready"]:
                        time.sleep(.1);continue
                    seat=next(i for i in range(len(state["players"])) if i not in data["taps"])
                page = [a, b][seat]
                state = api(host.uno, f"/api/lobbies/{code}", token(page))
                g, phase = state["game"], state["game"]["phase"]
                if phase == "OpeningColor":
                    page.get_by_role("button", name="Choose a color", exact=True).click()
                    button = page.get_by_role("dialog").get_by_role("button", name="Blue", exact=True)
                elif isinstance(phase, dict) and "Uno" in phase:
                    button = page.get_by_role("button", name="Call UNO!", exact=True)
                elif isinstance(phase,dict) and "TouchFive" in phase:
                    button=page.get_by_role("button",name="Tap the 5",exact=True)
                    expect(button).to_be_enabled()
                elif isinstance(phase,dict) and "Swap" in phase:
                    button=page.get_by_role("button",name="Keep my cards",exact=True)
                elif isinstance(phase, dict) and "Challenge" in phase:
                    button = page.get_by_role("button", name="Accept +4", exact=True)
                else:
                    legal = [c for c in g["hand"] if c["id"] in g["playable"] and not (c["value"] == "WildFour" and g["four_warning"])]
                    if legal:
                        card = legal[0]
                        if len(g["hand"]) == 2:
                            page.get_by_role("button", name="UNO!", exact=True).click()
                        tile = page.locator(f'.hand-card[data-card-id="{card["id"]}"]')
                        expect(tile).to_be_enabled()
                        expect(tile).to_have_class(re.compile(r'\bplayable\b'))
                        # Selection supports keyboard activation as well as tapping.
                        tile.focus()
                        tile.press("Enter")
                        if card["color"] is None:
                            card_control(page, "Play drawn card" if isinstance(phase, dict) and "Drawn" in phase else "Play card").click()
                            dialog = page.get_by_role("dialog", name="Choose a color")
                            if not canceled_wild:
                                dialog.get_by_role("button", name="Cancel", exact=True).click()
                                assert api(host.uno, f"/api/lobbies/{code}", token(page))["acceptedSequence"] == state["acceptedSequence"]
                                card_control(page, "Play drawn card" if isinstance(phase, dict) and "Drawn" in phase else "Play card").click()
                                canceled_wild = True
                            button = dialog.get_by_role("button", name="Blue", exact=True)
                        else:
                            button = card_control(page, "Play drawn card" if isinstance(phase, dict) and "Drawn" in phase else "Play card")
                    else:
                        button = card_control(page, "Keep card" if isinstance(phase, dict) and "Drawn" in phase else "Draw")
                try:
                    with page.expect_response(lambda r: r.url.endswith("/commands") and r.request.method == "POST") as pending:
                        button.click()
                except Exception:
                    print('FAILED UI STEP',step,'seat',seat,'phase',phase,'card',card if 'card' in locals() else None,'label',button.get_attribute('aria-label'),'selected',page.locator('.hand-card.selected').count(),flush=True)
                    page.screenshot(path=str(SHOTS/'failed-round.png'))
                    raise
                response = pending.value
                assert response.status == 200, response.text()
                expect(page.locator(".brand-button")).to_be_enabled()
            else:
                raise AssertionError("UI round failed to finish")
            expect(a.locator(".results-plane")).to_be_visible()
            expect(b.locator(".results-plane")).to_be_visible()
            screenshot(a, "results-desktop.png")
            screenshot(b, "results-phone.png")
            result_heading=a.locator('.results-plane h1').inner_text()
            assert ' · ' in result_heading and not any(m in result_heading for m in ['Ã','Â'])
            for _ in range(3):
                a.reload(wait_until='domcontentloaded')
                expect(a.locator('.results-plane h1')).to_have_text(result_heading)
            a.get_by_role('button',name='Back home',exact=True).click()
            expect(a.locator('.club-home')).to_be_visible()
            expect(a.locator('.uno-rules summary')).to_have_text('How to play · UnoGo')
            a.get_by_role('button',name='Match results',exact=True).first.click()
            for _ in range(3):
                a.reload(wait_until='domcontentloaded')
                expect(a.locator('.results-plane h1')).to_have_text(result_heading)
                assert a.evaluate("localStorage.getItem('unogo.review')")
            a.get_by_role('button',name='Back home',exact=True).click()
            a.reload(wait_until='domcontentloaded')
            expect(a.locator('.club-home')).to_be_visible()
            assert not a.evaluate("localStorage.getItem('unogo.review')")
            a.get_by_role('button',name='Match results',exact=True).first.click()
            print('PASS: UTF-8 result/footer text, repeated live-result and archive reload, explicit Home stays Home')
            assert api(host.uno, f'/api/stats/{state["players"][0]["userId"]}', token(a))["gamesPlayed"] == 1
            large_table_and_languages(host, a, base)
            registered_friend_invitation(host,browser,base,errors)
            assert not errors, errors
            # Resolve the currently served hashed bundle, not a historical dx path.
            entry=re.search(r'<script[^>]+src="([^"]+)"',(PUBLIC/'index.html').read_text()).group(1)
            script=(PUBLIC/entry.lstrip('/.')).read_text()
            wasm=PUBLIC/re.search(r'["\'](/[^"\']+\.wasm)["\']',script).group(1).lstrip('/.')
            manifest={"browser":"Microsoft Edge / Chromium","requiredViewports":["1440x960","390x844","320x740"],"captures":CAPTURES,"wasmSha256":hashlib.sha256(wasm.read_bytes()).hexdigest()}
            (SHOTS/"capture-manifest.json").write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf8")
            print(f"PASS: two-browser guest invite, keyboard/touch round, Wild cancellation, UNO, reactions, pause, offline lock, host/browser restart, results; {step} accepted UI actions")
            browser.close()
    finally:
        host.close()


if __name__ == "__main__":
    main()
