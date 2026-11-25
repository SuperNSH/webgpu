#!/usr/bin/env python3
"""Fail on a blank page among the tour's pictures (needs Pillow).

    python scripts/blank.py <dir of PNGs>

A page is blank when no pixel of where the demo is differs from the header's
background: a canvas that drew (its probe says so) and shows nothing.
"""

import sys
from pathlib import Path

from PIL import Image

# What it shows is downloaded: no page of ours is blank when that fails.
SKIPPED = {"ComputeToys"}


def blank(path):
    image = Image.open(path).convert("RGB")
    width, height = image.size
    page = image.getpixel((int(0.97 * width), int(0.04 * height)))
    return not any(
        any(abs(a - b) > 8 for a, b in zip(image.getpixel((x * width // 40, y * height // 80)), page))
        for x in range(1, 40)
        for y in range(12, 62)
    )


def main():
    pictures = [p for p in sorted(Path(sys.argv[1]).glob("*.png")) if p.stem not in SKIPPED]
    if not pictures:
        sys.exit(f"no pictures in {sys.argv[1]}")
    found = [p.name for p in pictures if blank(p)]
    print(f"{len(pictures)} pictures, blank: {', '.join(found) or 'none'}")
    sys.exit(1 if found else 0)


main()
