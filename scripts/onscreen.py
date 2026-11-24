#!/usr/bin/env python3
"""Run an example with `nsh run` on this desktop, look at the screen, and check the GPU drew.

    python scripts/onscreen.py <target: macos|linux|windows> <app dir> <output dir>

Takes two screenshots a second apart (needs Pillow) and passes when the red
triangle is on both, over the view its transparent canvas lies on, and the rotating cube moved between them. The log and
the screenshots stay in the output dir either way. `NSH` names the CLI.
"""

import os
import re
import subprocess
import sys
import time
from pathlib import Path

from PIL import ImageGrab

CONNECT_SECONDS = 2700  # the first run builds the host


def red(image):
    """Pixels of the triangle's red, give or take the display's color profile."""
    return sum(n for n, (r, g, b) in image.getcolors(1 << 24) if r > 200 and g < 80 and b < 80)


def through(image):
    """Pixels of the view under the triangle's transparent canvas (#3498db)."""
    return sum(n for n, (r, g, b) in image.getcolors(1 << 24) if r < 100 and 120 < g < 180 and b > 190)


def moved(a, b):
    """Pixels that are not the same in two screenshots."""
    return sum(1 for p, q in zip(a.getdata(), b.getdata()) if p != q)


def main():
    target, app, out = sys.argv[1], Path(sys.argv[2]).resolve(), Path(sys.argv[3]).resolve()
    out.mkdir(parents=True, exist_ok=True)
    log = out / f"{target}.log"
    with log.open("w") as sink:
        run = subprocess.Popen(
            [os.environ.get("NSH", "nsh"), "run", "--target", target, "--window-size", "700x560"],
            cwd=app, stdout=sink, stderr=subprocess.STDOUT,
        )
        try:
            end = time.time() + CONNECT_SECONDS
            while not re.search(r"connected, build", log.read_text(errors="replace")):
                if run.poll() is not None or time.time() > end:
                    sys.exit(f"the app never connected:\n{log.read_text(errors='replace')[-4000:]}")
                time.sleep(2)
            time.sleep(15)
            shots = []
            for n in (1, 2):
                shot = ImageGrab.grab().convert("RGB")
                shot.save(out / f"{target}-{n}.png")
                shots.append(shot)
                time.sleep(1)
        finally:
            run.terminate()
    reds, changed = [red(s) for s in shots], moved(*shots)
    print(f"{target}: red pixels {reds}, changed pixels {changed}")
    lines = [l for l in log.read_text(errors="replace").splitlines() if not l.startswith("PNG")]
    print("\n".join(l[:300] for l in lines[-40:]))
    if min(reds) < 2000:
        sys.exit("no red triangle on screen")
    if min(through(s) for s in shots) < 50000:
        sys.exit("the view under the transparent canvas does not show")
    if changed < 2000:
        sys.exit("the cube did not move")


if __name__ == "__main__":
    main()
