"""Render the authored SVG identity to platform icons. Requires Playwright and Pillow.

Run from any directory; no remote images, fonts or drawing services are used.
The vector masters remain the editable source of truth.
"""
from io import BytesIO
from pathlib import Path

from PIL import Image
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "crates/uno-client/assets/brand"


def main():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="msedge", headless=True)
        try:
            page = browser.new_page(device_scale_factor=2)

            def render(master, size, filename):
                page.set_viewport_size({"width": size, "height": size})
                svg = (BRAND / master).read_text(encoding="utf8")
                page.set_content(
                    '<style>html,body{margin:0;width:100%;height:100%;overflow:hidden}'
                    'svg{display:block;width:100%;height:100%}</style>' + svg
                )
                raster = Image.open(BytesIO(page.screenshot(omit_background=True)))
                raster = raster.resize((size, size), Image.Resampling.LANCZOS)
                raster.save(BRAND / filename, optimize=True)
                return raster

            for size in (180, 192, 512, 1024):
                filename = "apple-touch-icon-v2.png" if size == 180 else f"icon-{size}-v2.png"
                render("unogo-icon-v2.svg", size, filename)
            render("unogo-maskable-v2.svg", 512, "icon-maskable-512-v2.png")
            render("unogo-favicon-v2.svg", 32, "favicon-32-v2.png")
            favicon = render("unogo-favicon-v2.svg", 64, "favicon-64-v2.png")
            favicon.save(BRAND / "favicon-v2.ico", sizes=[(16, 16), (32, 32), (48, 48)])
        finally:
            browser.close()
    print("Rendered UnoGo icons: Apple touch, Android, maskable, favicon and 1024px master")


if __name__ == "__main__":
    main()
