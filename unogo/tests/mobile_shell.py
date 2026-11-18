"""Mobile chrome/account regression checks against an isolated release host.

Insets are simulated CSS variables, not evidence of testing on a real iPhone.
No production accounts or database are touched.
"""
import json
from pathlib import Path

from playwright.sync_api import sync_playwright, expect

import browser_flow as flow
from live_host import ROOT, api


def main():
    flow.PUBLIC = ROOT / "unogo/target/dx/uno-client/release/web/public"
    flow.BINARY = ROOT / "rust/target/release/domino-server.exe"
    host = flow.Host()
    shots = ROOT / "unogo/docs/screenshots/mobile-shell"
    shots.mkdir(parents=True, exist_ok=True)
    try:
        account = api(host.uno, "/api/auth/register", body={
            "username": "MobileLayout", "password": "isolated-layout-test-password"
        })
        with sync_playwright() as p:
            browser = p.chromium.launch(channel="msedge", headless=True)
            try:
                for width, height, top, bottom, label in [
                    (393, 852, 59, 34, "iphone-insets"),
                    (320, 568, 20, 0, "small-phone"),
                    (852, 393, 0, 21, "landscape"),
                    (1440, 1000, 0, 0, "desktop"),
                ]:
                    context = browser.new_context(viewport={"width": width, "height": height})
                    context.add_init_script(
                        "localStorage.setItem('unogo.token'," + json.dumps(account["token"]) + ");"
                        "localStorage.setItem('domino.language','en');"
                    )
                    page = context.new_page()
                    errors = []
                    page.on("pageerror", lambda error: errors.append(str(error)))
                    page.goto(f"http://127.0.0.1:{host.uno}", wait_until="networkidle")
                    page.evaluate("""([top,bottom])=>{
                        document.documentElement.style.setProperty('--uno-safe-top',top+'px');
                        document.documentElement.style.setProperty('--uno-safe-bottom',bottom+'px');
                    }""", [top, bottom])
                    expect(page.locator(".profile-button")).to_be_visible()
                    logo = page.locator(".brand-button").bounding_box()
                    assert logo["y"] >= top, (label, "header under status bar", logo)
                    nav = page.locator(".club-navigation")
                    bounds = nav.bounding_box()
                    assert bounds["y"] + bounds["height"] <= height - bottom - 11
                    assert nav.evaluate("e=>getComputedStyle(e).backgroundColor") == "rgba(16, 46, 80, 0.93)"
                    assert page.evaluate("document.documentElement.scrollWidth<=innerWidth")
                    page.screenshot(path=str(shots / f"{label}-home.png"))
                    # Open after scrolling to ensure the fixed panel ignores page position.
                    page.evaluate("scrollTo(0,document.body.scrollHeight)")
                    nav.get_by_role("button", name="Account", exact=True).click()
                    panel = page.locator(".account-sheet")
                    expect(panel).to_be_visible()
                    sheet = panel.bounding_box()
                    assert sheet["y"] >= top + 11
                    assert sheet["y"] + sheet["height"] <= height - bottom - 11
                    assert panel.evaluate("e=>getComputedStyle(e).backgroundColor") == "rgb(16, 43, 77)"
                    assert panel.evaluate("e=>getComputedStyle(e).paddingTop") == "0px"
                    page.screenshot(path=str(shots / f"{label}-account-open.png"))
                    close = page.get_by_role("button", name="Close account", exact=True)
                    close_y = close.bounding_box()["y"]
                    body = page.locator(".account-body")
                    page.get_by_text("Change password", exact=True).click()
                    page.locator("#new-password").fill("not-submitted-layout-check")
                    assert close.bounding_box()["y"] == close_y, (label, "close button scrolled away")
                    body.evaluate("e=>e.scrollTop=e.scrollHeight")
                    expect(panel.get_by_role("button", name="Sign out", exact=True)).to_be_in_viewport()
                    assert close.bounding_box()["y"] == close_y
                    assert panel.evaluate("e=>e.scrollWidth<=e.clientWidth")
                    page.screenshot(path=str(shots / f"{label}-account.png"))
                    close.click()
                    expect(panel).to_have_count(0)
                    nav.get_by_role("button", name="Friends", exact=True).click()
                    expect(page.locator(".friends-columns")).to_be_visible()
                    assert not errors, errors
                    context.close()
                    print(f"PASS: {label}: safe header/navigation, navy theme, bounded scrolling account, fixed close")
                context = browser.new_context(viewport={"width":393,"height":852})
                page = context.new_page()
                page.goto(f"http://127.0.0.1:{host.uno}", wait_until="networkidle")
                expect(page.locator(".auth-layout")).to_be_visible()
                page.evaluate("document.documentElement.style.setProperty('--uno-safe-top','59px')")
                assert page.locator(".brand-button").bounding_box()["y"] >= 59
                page.screenshot(path=str(shots / "iphone-insets-entry.png"))
                context.close()
                print("PASS: signed-out entry header respects the simulated status-bar inset")
            finally:
                browser.close()
    finally:
        host.close()


if __name__ == "__main__":
    main()
