# Iris documentation site

The static Astro Starlight site for `iris.gecode.dev`, described in
[PLAN.md §8.10](../PLAN.md#810-documentation-site-site) and delivered in
milestone M5. The home page connects a DIMACS input to its reader spec and
typed data,
then shows executable format examples. The format library renders every
format’s README, highlighted reader spec, and cases with their real output.
The Language Specification is rendered directly from `spec/`, with versioned
chapter pages and error-code links shared with the language server.
The introduction and planned documentation pages use the same shell.

## Build and check

Use Node 22.12 or newer, npm, and the stable Rust toolchain. Asset preparation
also requires `rsvg-convert`:
install `librsvg` with Homebrew or `librsvg2-bin` on Ubuntu.

In this directory:

```sh
npm ci
npm run build
npm run check
npx astro check
```

`check` first compares all example text and downloads with the current local
CLI, then checks the hero’s UTF-8 ranges and data relationships. It also runs
Astro diagnostics, checks internal links and anchors against
`dist/`, validates all built HTML, and checks text/control colour contrast.
The Specification check compares prose, examples and grammar with their sources,
requires every chapter link and heading anchor, and checks every registered
error anchor and every error code the language server can link to.
Build first: checks do not silently rebuild. They do not crawl external sites.
The contrast table includes both themes, the rail, terminal, and syntax
highlighting;
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

The sidebar in `astro.config.mjs` references pages under `src/content/docs/`
and discovers the Formats section from `formats/` and the Language Specification
section from `spec/`. The splash lives in
`src/pages/index.astro`; `src/pages/formats/` generates the format index and
individual pages. Keep stubs honest until their owning task supplies content.

`scripts/specification-sources.mjs` discovers numeric version directories under
`spec/` and follows each README's chapter index for page and sidebar order.
`src/pages/specification/` renders each Markdown file and `grammar.ebnf` at
`/specification/VERSION/FILE/`; the README is the version's chapter index.
`/specification/` redirects to the highest numeric version. Adding `spec/1.0/`
with its chapter index needs no site code change. The native version links
retain the current file when available, otherwise lead to the chapter index.

The Markdown loader and repository-link resolver are shared with the format
library. Links within the Specification lead to its pages; other repository
links retain their GitHub destinations. GitHub-style heading IDs preserve the
source's existing fragments. Error table codes gain lowercase self-links such
as `/specification/0.1/errors/#e-check-failed`. The source text is unchanged;
smart punctuation is disabled. Complete fences use the site's Expressive Code
component and Iris TextMate grammar; EBNF has its own highlighting. Prose has a
comfortable width and tables and code scroll within that width on phones.
Astro's Vite configuration supplies the repository source root to bundled
prerender modules, so they can load files without copying them into `site/`.

`npm run check:specification` checks the existing build without rewriting it.
The ordinary link, HTML and contrast checks include all Specification pages.

Tailwind 4 runs through `@tailwindcss/vite`, with `@astrojs/starlight-tailwind`
compatibility and the documented `base, starlight, theme, components, utilities`
layer order. `src/styles/custom.css` defines the colour, font, and spacing
tokens in `@theme`, maps Starlight’s theme roles, and applies utilities to
stock components. Header, Sidebar, and the home Hero are overridden. Starlight
owns
search, theme selection, native mobile navigation, and documentation tabs.
The home showcase enhances fragment links into keyboard-accessible tabs; its
leaf URLs restore the format and representation on reload and Back/Forward.
The hero animates once, settles on interaction, and starts complete when reduced
motion is requested.
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

## Executable examples

Every dev/build also runs `scripts/prepare-examples.mjs`. Cargo builds the local
`iris` and `iris-driver` binaries; their executable paths come from Cargo’s
artifact messages. `CARGO_TARGET_DIR` is respected. No installed Iris binary is
used. The script discovers every format and case, including errors,
parameters, summaries, and job shop/Sudoku catalogs. It checks canonical JSON
against each successful fixture and error codes/locations against error fixtures.

The ignored `src/generated/formats.ts` and `public/examples/` hold the results.
`src/data/formats.ts` exposes the records for build-time use by home and format
pages. Downloads include exact source and output bytes, complete case bundles,
and the existing Gecode and MIT notices. Long code panels display a bounded,
labelled excerpt with a full-file link; JSON remains the CLI’s text.

`npm run check:examples` collects current results without writing them and checks
the generated module, public downloads, built downloads, and marked HTML code.
It requires each format page and every case’s input and outputs, plus the
rendered README, source commit, and format index.
Missing, extra, or changed output fails. Build first after changing a source or
the CLI. `npm run check:hero` checks the authored DIMACS mapping against the
current CLI and the built entry view. Both checks are part of `npm run check`.
CI installs Rust and caches Cargo artifacts before running these same commands.

Adding a format or case under `formats/` needs no site change. The library
convention is `<directory>/<directory>.iris`, a README with a title, description
paragraph and pinned Gecode source link, and cases following the conformance
contract. `scripts/format-sources.mjs` derives titles, descriptions and Gecode
commits from READMEs, renders their Markdown, and rewrites repository links to
site pages/downloads or GitHub. The first successful case becomes a new format’s
home example; the existing six retain their curated choices and URLs, including
`jobshop` at `/formats/job-shop/`. A format containing only error cases still
gets a library page. Case titles come from `case.toml`.

Markdown and TOML use the same pinned parsers already present in the dependency
tree, declared directly for these imports. Use the example marker
attributes from `ExamplePane.astro` and `ExampleTerminal.astro` when rendering
these records on another page, so their output enters the freshness check.
