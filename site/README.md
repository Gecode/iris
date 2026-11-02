# Iris documentation site

The static Astro Starlight site for `iris.gecode.dev`, described in
[PLAN.md §8.10](../PLAN.md#810-documentation-site-site) and delivered in
milestone M5. The scaffold has an introduction, a minimal splash, and planned
pages for every documentation section. Later tasks supply the full content.

## Build and check

Use Node 22.12 or newer and npm. Asset preparation also requires `rsvg-convert`:
install `librsvg` with Homebrew or `librsvg2-bin` on Ubuntu.

In this directory:

```sh
npm ci
npm run build
npm run check
npx astro check
```

`check` runs Astro diagnostics, checks internal links and anchors against
`dist/`, validates all built HTML, and checks text/control colour contrast.
Build first: checks do not silently rebuild. They do not crawl external sites.
The contrast table includes both themes, the rail, and syntax highlighting;
original logo colours are reported as decorative, not used as a text palette.
These checks support accessibility review; they do not prove WCAG conformance.

```sh
npm run dev
npm run preview -- --host 127.0.0.1
```

Search uses Pagefind and works in the built preview. The development server
shows Starlight’s search warning. Build and checks also run in the
[site workflow](../.github/workflows/site.yml). Pushes to `main` deploy the
checked build to the Cloudflare Worker `iris-site` once its repository secrets
are configured. See [publishing](PUBLISHING.md) for the one-time Gecode account
and custom-domain setup, deployment, verification and rollback.

## Content and styling

The explicit sidebar in `astro.config.mjs` references pages under
`src/content/docs/`. The splash lives in `src/pages/index.astro`. Keep stubs
honest until their owning task supplies content; the Specification placeholder
is not normative text. Its explicit slug preserves the `0.1` route.

Tailwind 4 runs through `@tailwindcss/vite`, with `@astrojs/starlight-tailwind`
compatibility and the documented `base, starlight, theme, components, utilities`
layer order. `src/styles/custom.css` defines the colour, font, and spacing
tokens in `@theme`, maps Starlight’s theme roles, and applies utilities to
stock components. Only Header and Sidebar are overridden; Starlight owns
search, theme selection, native mobile navigation, and tabs.
The semantic colour mapping follows the compatibility package in the utilities
layer so its default gray palette cannot replace the checked Iris tones.

The Iris grammar is imported directly from
[`editors/textmate/iris.tmLanguage.json`](../editors/textmate/iris.tmLanguage.json),
with only its registration name adjusted to `iris`. Do not add a copied grammar.
The two small syntax themes are in `src/code-themes.mjs`.

`public/fonts/` contains the six self-hosted font files and their licenses;
see its [font notes](public/fonts/README.md). CSS declares their actual weights:
Raleway 500 for headings/navigation, Open Sans 400 for body text, Roboto Mono
400 for code. No remote fonts are fetched.

## Generated assets

Every dev/build runs `scripts/prepare-assets.mjs`. It copies the selected
unchanged [brand assets](../brand/README.md) and complete favicon set into
`public/`, and renders `og-image.png` at 1200×630 from the existing lockup.
Those files, `.astro/`, `dist/`, and `node_modules/` are ignored. The preparation
script resolves its paths from its own location and never writes to `brand/`.
Fonts and licenses are source assets and must be retained with the site.

HTML validation permits generated inline styles and formatting conventions.
The checker treats only Expressive Code’s line wrappers and copy icon divs as
phrasing content; nesting checks stay active everywhere else. The 404 page
sets its canonical and open graph URL to the emitted `/404.html` explicitly.

Later tasks extend `npm run check` with example freshness when they add CLI
output, and extend CI path filters when builds begin consuming formats, the
Specification, or Rust sources.
