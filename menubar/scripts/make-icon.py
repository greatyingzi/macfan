#!/usr/bin/env python3
"""Generate the app icon (SVG geometry -> HTML) and render it to PNG.

Values are computed rather than eyeballed, so the drawing can be checked
numerically: Big Sur icon grid (824x824 squircle on a 1024 canvas, 185.5 corner
radius), fan petals on exact arcs, hub on exact radii.
"""
import math
import pathlib
import subprocess
import sys

OUT = pathlib.Path("/Users/viture/.hermes/cache/scratch/icon")
OUT.mkdir(parents=True, exist_ok=True)

CANVAS = 1024
INSET = 100.0                    # Big Sur: 824x824 artwork inside 1024
SIDE = CANVAS - 2 * INSET        # 824
CORNER = 185.5                   # Big Sur corner radius
CENTER = CANVAS / 2.0

PETAL_RADIUS = 205.0             # distance of the petal arc from the centre
PETAL_STROKE = 112.0             # petal thickness
PETAL_SPAN = 62.0                # degrees each petal sweeps
PETAL_PHASE = -20.0              # tilt: makes the fan read as spinning
HUB_RADIUS = 80.0
HUB_RING = 34.0                  # inner hole, in the background colour


def polar(radius: float, degrees: float) -> tuple[float, float]:
    rad = math.radians(degrees)
    return CENTER + radius * math.cos(rad), CENTER + radius * math.sin(rad)


def petal_path(phase: float) -> str:
    x1, y1 = polar(PETAL_RADIUS, phase - PETAL_SPAN / 2)
    x2, y2 = polar(PETAL_RADIUS, phase + PETAL_SPAN / 2)
    return f"M {x1:.2f} {y1:.2f} A {PETAL_RADIUS} {PETAL_RADIUS} 0 0 1 {x2:.2f} {y2:.2f}"


def build_svg() -> str:
    petals = "\n      ".join(
        f'<path d="{petal_path(PETAL_PHASE + i * 90)}"/>' for i in range(4)
    )
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}"
     viewBox="0 0 {CANVAS} {CANVAS}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#63CDFB"/>
      <stop offset="45%" stop-color="#2F7BE8"/>
      <stop offset="100%" stop-color="#12408F"/>
    </linearGradient>
    <radialGradient id="sheen" cx="0.3" cy="0.16" r="0.62">
      <stop offset="0%" stop-color="#ffffff" stop-opacity="0.34"/>
      <stop offset="100%" stop-color="#ffffff" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="clip">
      <rect x="{INSET}" y="{INSET}" width="{SIDE}" height="{SIDE}"
            rx="{CORNER}" ry="{CORNER}"/>
    </clipPath>
  </defs>

  <g clip-path="url(#clip)">
    <rect x="{INSET}" y="{INSET}" width="{SIDE}" height="{SIDE}" fill="url(#bg)"/>
    <rect x="{INSET}" y="{INSET}" width="{SIDE}" height="{SIDE}" fill="url(#sheen)"/>
  </g>

  <g stroke="#ffffff" stroke-width="{PETAL_STROKE}" stroke-linecap="round"
     fill="none" stroke-opacity="0.96">
      {petals}
  </g>
  <circle cx="{CENTER}" cy="{CENTER}" r="{HUB_RADIUS}" fill="#ffffff"
          fill-opacity="0.96"/>
  <circle cx="{CENTER}" cy="{CENTER}" r="{HUB_RING}" fill="#2059B4"/>
</svg>"""


def main() -> int:
    svg = build_svg()
    (OUT / "icon.svg").write_text(svg)
    html = f"""<!doctype html><meta charset="utf-8">
<style>
  html,body {{ margin:0; padding:0; background:transparent; }}
  svg {{ display:block; }}
</style>
{svg}
"""
    (OUT / "icon.html").write_text(html)

    renderer = pathlib.Path(
        "/Users/viture/.hermes/skills/creative/html-to-image-and-pdf/scripts/html_render.py"
    )
    cmd = [
        sys.executable, str(renderer),
        "--html", str(OUT / "icon.html"),
        "--out", str(OUT / "icon-1024.png"),
        "--width", str(CANVAS), "--height", str(CANVAS),
        "--scale", "1", "--viewport", "--transparent",
    ]
    print("rendering:", " ".join(cmd[1:]))
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(result.stdout.strip() or result.stderr.strip())
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
