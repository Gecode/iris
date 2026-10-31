# brand

The Iris logo, wordmarks, and icons. [PLAN.md §8.11](../PLAN.md#811-brand-logo-and-favicon-brand)
describes the brand. This work belongs to milestone M0.

## The mark

The mark is an iris drawn with the cards from the Gecode logo. The orange
card is the standard, the red and blue cards are the falls, and the green
stem carries the flower. The corners that meet at the center are sharper
than the outer corners.

| File | Use |
|---|---|
| `iris-mark.svg` | The mark in color |
| `iris-mark-ink.svg` | One color, for light backgrounds |
| `iris-mark-white.svg` | One color, for dark backgrounds |
| `iris-logo-stacked.svg` | The mark above the wordmark |
| `iris-logo-horizontal.svg` | The mark beside the wordmark |

Each lockup comes in four versions:
- no suffix: color mark, ink text;
- `-dark`: color mark, white text, for dark backgrounds;
- `-ink`: all ink;
- `-white`: all white.

`png/` has PNG renderings of the mark at 256, 512, and 1024 pixels and
of every lockup at 512 pixels high.

## Icons

`favicon/` holds the icon set:
- `favicon.svg`;
- `favicon.ico` (16, 32, and 48 pixels) and the matching PNGs;
- `apple-touch-icon.png` (180 pixels);
- `icon-192.png` and `icon-512.png`;
- `site.webmanifest`.

The icons use a tuned mark with wider gaps and a thicker stem, so that the
parts stay apart at 16 and 32 pixels. `app-icon.svg` places that mark on a
white square for the touch and manifest icons.

## Palette

| Role | Color |
|---|---|
| Red card | `#DA251D` |
| Orange card | `#EB891B` |
| Blue card | `#005CA1` |
| Green stem | `#0B7646` |
| Ink (wordmark) | `#27393E` |

Ink text on white has a contrast ratio of 12.1:1. White text on GitHub's
dark background `#0D1117` has 18.9:1.

## Rules for use

- The wordmark is always lowercase `iris`, set in Raleway SemiBold and
  converted to outlines. In prose, write Iris; never write the name in
  capitals.
- Keep clear space around the mark of at least the stem's width on every
  side.
- Use the mark at 16 pixels or more, and use the icons from `favicon/`
  below 48 pixels. Use a lockup only where the wordmark is at least 10
  pixels high.
- Do not recolor, rotate, stretch, or rearrange the cards. On a dark or
  busy background, use the `-dark` or `-white` version.

## Regenerating

`build.py` generates every file in this directory except this README. It
needs the variable Raleway font from
[Google Fonts](https://github.com/google/fonts/tree/main/ofl/raleway)
(SIL Open Font License), `uv`, `rsvg-convert`, and ImageMagick:

```sh
uv run --with fonttools python brand/build.py path/to/Raleway[wght].ttf
```

`concepts/` holds the generated concept images that led to the mark. It is
not tracked.
