"""Screenshot baselines for the operator interface (design brief §8, V02 and W01).

Captures fixed-size images of the fixture page and the service window with sample
data, fonts loaded, the clock frozen, animations off and a known scale, then compares
them with the approved images in docs/design/baselines/.

    cd ui && npm run build && npx vite preview --port 4400 &
    python tests/ui/baselines.py capture   # first time, or after an approved change
    python tests/ui/baselines.py check     # fails on any unreviewed change

A changed screenshot is not approved by re-capturing it: review the diff image in
docs/design/baselines/diff/, record the decision in signoff.md, then capture.
These are browser captures; Windows client captures belong beside them.
"""

import asyncio
import pathlib
import sys

from PIL import Image, ImageChops
from playwright.async_api import async_playwright

URL = "http://localhost:4400/"
ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "docs/design/baselines"
SIZES = [(1440, 900), (1280, 720), (1092, 614), (800, 560)]
FROZEN = "2026-11-01T09:41:00"


async def shots(page_dir: pathlib.Path):
    page_dir.mkdir(parents=True, exist_ok=True)
    async with async_playwright() as p:
        b = await p.chromium.launch()
        for theme in ("dark", "light"):
            for w, h in SIZES:
                ctx = await b.new_context(viewport={"width": w, "height": h}, device_scale_factor=1, reduced_motion="reduce")
                pg = await ctx.new_page()
                await pg.clock.install(time=FROZEN)
                await pg.add_init_script(f"localStorage.setItem('wp-ui-theme', '{theme}')")
                await pg.goto(URL + "#/control")
                await pg.evaluate("document.fonts.ready")
                await pg.wait_for_timeout(600)
                await pg.screenshot(path=str(page_dir / f"service-{w}x{h}-{theme}.png"))
                await ctx.close()
        ctx = await b.new_context(viewport={"width": 1440, "height": 900}, reduced_motion="reduce")
        pg = await ctx.new_page()
        await pg.clock.install(time=FROZEN)
        await pg.goto(URL + "#/fixture")
        await pg.evaluate("document.fonts.ready")
        await pg.wait_for_timeout(400)
        await pg.screenshot(path=str(page_dir / "fixture.png"), full_page=True)
        await b.close()


def compare() -> int:
    tmp = OUT / "current"
    asyncio.run(shots(tmp))
    (OUT / "diff").mkdir(exist_ok=True)
    failed = 0
    for cur in sorted(tmp.glob("*.png")):
        ref = OUT / cur.name
        if not ref.exists():
            print(f"NEW      {cur.name} (no approved baseline yet)")
            failed += 1
            continue
        a, b = Image.open(ref).convert("RGB"), Image.open(cur).convert("RGB")
        if a.size != b.size:
            print(f"CHANGED  {cur.name} size {a.size} -> {b.size}")
            failed += 1
            continue
        d = ImageChops.difference(a, b)
        box = d.getbbox()
        if box:
            d.point(lambda v: 255 if v else 0).save(OUT / "diff" / cur.name)
            print(f"CHANGED  {cur.name} in area {box}; see diff/{cur.name}")
            failed += 1
        else:
            print(f"same     {cur.name}")
    return 1 if failed else 0


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "capture":
        asyncio.run(shots(OUT))
        print("Captured. Record the review in docs/design/baselines/signoff.md.")
    else:
        sys.exit(compare())
