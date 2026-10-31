"""Generate the Iris mark, lockups, and icons.

Usage: uv run --with fonttools python build.py RALEWAY_TTF [OUT_DIR]

RALEWAY_TTF is the variable Raleway font from Google Fonts. OUT_DIR
defaults to this script's directory. rsvg-convert and ImageMagick
(magick) render the PNG and ICO files.

The mark is three Gecode cards and a stem. Cards are rounded squares
rotated by 45 degrees. Geometry is given in the rotated frame, where X'
points down-right and Y' points down-left; the orange card is centered
at the origin.
"""

import math
import subprocess
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

RED = "#DA251D"
ORANGE = "#EB891B"
BLUE = "#005CA1"
GREEN = "#0B7646"
INK = "#27393E"
WHITE = "#FFFFFF"

SQRT2 = math.sqrt(2)


class Mark:
    """Mark geometry; the default is the logo, tuned variants are icons."""

    def __init__(self, gap=2.0, stem_gap=3.5, stem_width=7.5,
                 orange_half=19.0, side_half=16.0, side_shift=4.0,
                 orange_radius=7.0, side_radius=6.0, inner_radius=1.5,
                 stem_bottom=60.8):
        self.gap = gap
        self.stem_gap = stem_gap
        self.inner_radius = inner_radius
        self.stem_width = stem_width
        self.orange_half = orange_half
        self.side_half = side_half
        self.side_shift = side_shift
        self.orange_radius = orange_radius
        self.side_radius = side_radius
        self.stem_bottom = stem_bottom

    @property
    def side_offset(self):
        """Rotated-frame distance from the orange center to a side card."""
        return self.orange_half + self.gap + self.side_half

    def side_center(self, sign):
        """Center of the red (sign -1) or blue (sign +1) card."""
        a, b = self.side_shift, self.side_offset
        x_rot, y_rot = (a, b) if sign < 0 else (b, a)
        return (x_rot - y_rot) / SQRT2, (x_rot + y_rot) / SQRT2

    @staticmethod
    def reach(half, radius):
        """Distance from a card center to its rounded tip."""
        return half * SQRT2 - radius * (SQRT2 - 1)

    def bbox(self):
        side_x, side_y = self.side_center(+1)
        side_reach = self.reach(self.side_half, self.side_radius)
        left = -(side_x + side_reach)
        top = -self.reach(self.orange_half, self.orange_radius)
        bottom = max(self.stem_bottom, side_y + side_reach)
        return left, top, -left, bottom

    def stem_path(self):
        """Stem with a pointed top that follows the side cards' gaps."""
        edge = self.side_shift + self.side_half + self.stem_gap
        tip = SQRT2 * edge
        half = self.stem_width / 2
        shoulder = tip + half
        straight = self.stem_bottom - half
        return (f"M0 {tip:.3f}L{half:.3f} {shoulder:.3f}V{straight:.3f}"
                f"A{half:.3f} {half:.3f} 0 0 1 {-half:.3f} {straight:.3f}"
                f"V{shoulder:.3f}Z")

    @staticmethod
    def card(center_rot, half, radii, fill):
        """Rounded square in the rotated frame.

        radii gives the visual top, right, bottom, and left corners.
        """
        cx, cy = center_rot
        x0, y0, x1, y1 = cx - half, cy - half, cx + half, cy + half
        top, right, bottom, left = radii

        def arc(r, x, y):
            return f"A{r:.3f} {r:.3f} 0 0 1 {x:.3f} {y:.3f}"

        d = (f"M{x0 + top:.3f} {y0:.3f}H{x1 - right:.3f}"
             f"{arc(right, x1, y0 + right)}V{y1 - bottom:.3f}"
             f"{arc(bottom, x1 - bottom, y1)}H{x0 + left:.3f}"
             f"{arc(left, x0, y1 - left)}V{y0 + top:.3f}"
             f"{arc(top, x0 + top, y0)}Z")
        return f'<path d="{d}" fill="{fill}"/>'

    def svg(self, colors):
        """Mark elements centered at the orange card; colors is r, o, b, g."""
        red, orange, blue, green = colors
        a, b = self.side_shift, self.side_offset
        outer, side, inner = (self.orange_radius, self.side_radius,
                              self.inner_radius)
        # Corners that meet at the center of the flower are sharper.
        cards = "".join([
            self.card((0, 0), self.orange_half,
                      (outer, outer, inner, outer), orange),
            self.card((a, b), self.side_half,
                      (inner, inner, side, side), red),
            self.card((b, a), self.side_half,
                      (inner, side, side, inner), blue),
        ])
        return (f'<g transform="rotate(45)">{cards}</g>'
                f'<path d="{self.stem_path()}" fill="{green}"/>')


COLOR = (RED, ORANGE, BLUE, GREEN)


def one_color(color):
    return (color,) * 4


class Wordmark:
    """Lowercase "iris" as outlines in font units (y up, baseline 0)."""

    def __init__(self, font_path, weight, text="iris"):
        font = TTFont(font_path)
        font = instantiateVariableFont(font, {"wght": weight})
        glyph_set = font.getGlyphSet()
        cmap = font.getBestCmap()
        self.x_height = font["OS/2"].sxHeight
        pens = []
        x = 0
        for char in text:
            name = cmap[ord(char)]
            pen = SVGPathPen(glyph_set)
            glyph_set[name].draw(TransformPen(pen, (1, 0, 0, -1, x, 0)))
            pens.append(pen.getCommands())
            x += glyph_set[name].width
        self.advance = x
        self.path = " ".join(pens)
        bounds = font["glyf"]
        ymax = max(bounds[cmap[ord(c)]].yMax for c in text)
        xmin = bounds[cmap[ord(text[0])]].xMin
        last = cmap[ord(text[-1])]
        xmax = self.advance - glyph_set[last].width + bounds[last].xMax
        self.ink_left, self.ink_right, self.top = xmin, xmax, ymax

    def svg(self, x, baseline, scale, fill):
        return (f'<path transform="translate({x:.3f} {baseline:.3f}) '
                f'scale({scale:.5f})" d="{self.path}" fill="{fill}"/>')


def document(view, body, title, background=None):
    x, y, w, h = view
    back = ""
    if background:
        back = (f'<rect x="{x:.3f}" y="{y:.3f}" width="{w:.3f}" '
                f'height="{h:.3f}" fill="{background}"/>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" '
            f'viewBox="{x:.3f} {y:.3f} {w:.3f} {h:.3f}" role="img">'
            f'<title>{title}</title>{back}{body}</svg>\n')


def padded(bbox, pad):
    left, top, right, bottom = bbox
    return (left - pad, top - pad, right - left + 2 * pad,
            bottom - top + 2 * pad)


def lockups(mark, word, colors, text_fill):
    """Stacked and horizontal lockups; returns {name: (view, body)}."""
    left, top, right, bottom = mark.bbox()
    mark_height = bottom - top
    mark_body = mark.svg(colors)

    # Stacked: the wordmark's x-height is 24% of the mark height.
    scale = 0.24 * mark_height / word.x_height
    width = (word.ink_right - word.ink_left) * scale
    gap = 0.14 * mark_height
    baseline = bottom + gap + word.top * scale
    x = -width / 2 - word.ink_left * scale
    stacked = (padded((min(left, -width / 2), top, max(right, width / 2),
                       baseline), 0.08 * mark_height),
               mark_body + word.svg(x, baseline, scale, text_fill))

    # Horizontal: the x-height is 32% of the mark height and the baseline
    # sits on the bottom tips of the red and blue cards.
    scale = 0.32 * mark_height / word.x_height
    _, side_y = mark.side_center(+1)
    baseline = side_y + mark.reach(mark.side_half, mark.side_radius)
    x = right + 0.16 * mark_height - word.ink_left * scale
    text_right = x + word.ink_right * scale
    horizontal = (padded((left, top, text_right, bottom), 0.08 * mark_height),
                  mark_body + word.svg(x, baseline, scale, text_fill))
    return {"stacked": stacked, "horizontal": horizontal}


def write(path, text):
    path.write_text(text)
    print(path)


def render(svg, png, width=None, height=None, background=None):
    args = ["rsvg-convert", str(svg), "-o", str(png)]
    if width:
        args += ["-w", str(width)]
    if height:
        args += ["-h", str(height)]
    if background:
        args += ["-b", background]
    subprocess.run(args, check=True)
    print(png)


def square_view(mark, margin):
    left, top, right, bottom = mark.bbox()
    size = max(right - left, bottom - top) + 2 * margin
    cy = (top + bottom) / 2
    return (-size / 2, cy - size / 2, size, size)


MANIFEST = """{
  "name": "Iris",
  "short_name": "Iris",
  "icons": [
    { "src": "icon-192.png", "type": "image/png", "sizes": "192x192" },
    { "src": "icon-512.png", "type": "image/png", "sizes": "512x512" }
  ],
  "theme_color": "#27393E",
  "background_color": "#FFFFFF",
  "display": "standalone"
}
"""

# Raleway SemiBold; gecode.dev sets text in Raleway.
WORDMARK_WEIGHT = 600


def main(font_path, out):
    out = Path(out)
    png = out / "png"
    icons = out / "favicon"
    png.mkdir(parents=True, exist_ok=True)
    icons.mkdir(parents=True, exist_ok=True)
    word = Wordmark(font_path, WORDMARK_WEIGHT)
    mark = Mark()

    mark_view = padded(mark.bbox(), 0)
    variants = {
        "": (COLOR, INK),
        "-ink": (one_color(INK), INK),
        "-white": (one_color(WHITE), WHITE),
        "-dark": (COLOR, WHITE),
    }
    for suffix, (colors, text_fill) in variants.items():
        if suffix != "-dark":
            write(out / f"iris-mark{suffix}.svg",
                  document(mark_view, mark.svg(colors), "Iris"))
        for name, (view, body) in lockups(mark, word, colors,
                                          text_fill).items():
            write(out / f"iris-logo-{name}{suffix}.svg",
                  document(view, body, "Iris"))

    # Icons use wider gaps and a thicker stem so that the parts stay
    # separate at 16 and 32 pixels.
    icon = Mark(gap=4.0, stem_width=11.0, side_shift=5.0)
    write(icons / "favicon.svg",
          document(square_view(icon, 1.0), icon.svg(COLOR), "Iris"))
    write(icons / "app-icon.svg",
          document(square_view(icon, 14.0), icon.svg(COLOR), "Iris",
                   background=WHITE))
    write(icons / "site.webmanifest", MANIFEST)

    for size in (16, 32, 48):
        render(icons / "favicon.svg", icons / f"favicon-{size}.png",
               size, size)
    subprocess.run(["magick"] + [str(icons / f"favicon-{s}.png")
                                 for s in (16, 32, 48)]
                   + [str(icons / "favicon.ico")], check=True)
    print(icons / "favicon.ico")
    render(icons / "app-icon.svg", icons / "apple-touch-icon.png", 180, 180)
    for size in (192, 512):
        render(icons / "app-icon.svg", icons / f"icon-{size}.png",
               size, size)

    for size in (256, 512, 1024):
        render(out / "iris-mark.svg", png / f"iris-mark-{size}.png",
               height=size)
    for name in ("stacked", "horizontal"):
        for suffix in variants:
            render(out / f"iris-logo-{name}{suffix}.svg",
                   png / f"iris-logo-{name}{suffix}.png", height=512)


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        sys.exit("Usage: build.py RALEWAY_TTF [OUT_DIR]")
    main(sys.argv[1],
         sys.argv[2] if len(sys.argv) == 3 else Path(__file__).parent)
