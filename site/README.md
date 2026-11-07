# Iris documentation site

The static Astro Starlight site for `iris.gecode.dev`, described in
[PLAN.md §8.10](../PLAN.md#810-documentation-site-site) and delivered in
milestone M5. The home page connects a DIMACS input to its reader spec and
typed data,
then shows executable format examples. The format library renders every
format’s README, highlighted reader spec, and cases with their real output.
The Language Specification is rendered directly from `spec/`, with versioned
chapter pages and error-code links shared with the language server.
The introduction, tutorial, guides, tools, Rust, Python, TypeScript, C++ and Java pages use the same
shell, and so do the Python API reference that pdoc generates, the C++ API
reference that Doxygen generates, and the Java and Kotlin API references that
Javadoc and Dokka generate.
The other implementations page relates the implementations and Gecode.

## Build and check

Use Node 22.12 or newer, npm, the stable Rust toolchain, and
[uv](https://docs.astral.sh/uv/), which builds the Python package for its API
reference. The C++ API reference requires
[Doxygen](https://www.doxygen.nl/) 1.9.8 or newer: install `doxygen` with
Homebrew or apt. The Java and Kotlin API references require Java 27, as `java`
on `PATH` or in `JAVA_HOME`, for the Gradle build in `java/`. Asset preparation also requires `rsvg-convert`:
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

The same collector also prepares `src/generated/documentation.ts` for the tools
and Rust pages. `scripts/documentation-examples.mjs` runs the CLI commands,
formats a before/after pair extracted from `docs/formatting.md`, and extracts the
umbrella crate's example from its doc-tested source. Each collection executes
that Rust doctest. Setup commands are checked against their owning READMEs;
conformance uses the driver path reported by Cargo while showing the default
repository-relative path. In-place formatting runs on a disposable copy.
`CheckedExample.astro` marks commands, source snippets and outputs for the
existing freshness checker, which requires each page's examples and rejects
unmarked code blocks on these pages. The introduction reuses the format markers.


The tutorial and guides extend that collector through
`scripts/learning-examples.mjs`. Intermediate reader specs and inputs live in
`examples/tutorial/` and `examples/guides/`; final tutorial readers come directly
from the format library. Every collection checks each reader spec with the real
CLI and checks formatting under `examples/`. It runs every displayed read,
selection and catalog command, including expected failures. For each failure,
the conformance driver also checks the expected error code, and the user CLI
must emit no partial data. The same rendered-text markers cover all these
pages, including diagnostics and the canonical error document in the errors
guide. To add an example, register its required markers in `learningPages`,
collect its sources and command result, then render them with `CheckedExample`.

## Rust guide

`/using/rust/` shows the umbrella crate's doc-tested example, collected as above,
and the spec builders in `rust/gecode-iris/examples` with their recorded output,
collected by `scripts/rust-examples.mjs` with the shared
`scripts/example-files.mjs`. The collector requires every file in that directory
to appear on the page. The site build compiles none of them: the
`format_builders` test in `rust/` runs each program and compares its output, and
reads every format case through each builder; see the
[gecode-iris-build README](../rust/iris-build/README.md).

## Python guide and API reference

`/using/python/` renders the examples in `python/examples` with their recorded
output through the same `CheckedExample` markers, collected by
`scripts/python-examples.mjs` with the shared `scripts/example-files.mjs`. The
collector requires every file in that directory to appear on the page, checks
that the install commands still appear in `python/README.md`, and runs each
`.sh` example with the Rust CLI to check its recording. The Python side runs every example against the built package
in `python/tests/test_examples.py`; see the
[Python README](../python/README.md#guide-examples).

Every dev/build runs `scripts/prepare-python-api.mjs`. It builds a wheel from
`python/` with `uvx maturin@1.16.0 build`, respecting `CARGO_TARGET_DIR`,
installs it with the pinned pdoc of the `docs` dependency group into a
temporary uv environment on Python 3.15, and runs `python/docs/reference.py`.
uv downloads Python 3.15 when it is not installed. The ignored
`src/generated/python-api.json` holds the HTML fragment and its outline;
`src/pages/using/python/api.astro` places it in a Starlight page, and the
`.iris-python-api` rules in `src/styles/custom.css` style pdoc's markup.
`npm run check:python-api`, part of `npm run check`, compares the built page
with that output and requires the public classes, methods and attributes,
including the stub signature of `Spec.read`. The
[Python README](../python/README.md#api-reference) records why pdoc was chosen.

## TypeScript guide

`/using/typescript/` renders the examples in `typescript/examples` the same
way, collected by `scripts/typescript-examples.mjs`. The collector requires
every file in that directory to appear on the page and checks that the install
commands and the Content-Security-Policy line still appear in
`typescript/README.md`. The site build does not need the TypeScript package:
`typescript/test/examples.test.ts`, part of `npm test` in `typescript/`,
type-checks every example against the built package and runs the ones with a
recorded output; see the [TypeScript README](../typescript/README.md#tests).

## C++ guide and API reference

`/using/cpp/` renders the examples in `cpp/examples` the same way, collected
by `scripts/cpp-examples.mjs`, which also checks that the install command and
the CMake snippets still appear in `cpp/README.md`. The site build compiles no
C++: the `example-` tests in `cpp/` compile and run every example and compare
its output; see the [C++ README](../cpp/README.md#guide-examples).

Every dev/build runs `scripts/prepare-cpp-api.mjs`. It runs `doxygen` with
`cpp/Doxyfile`, the file the `iris-docs` target uses, so a warning in the public
headers fails the build, and so does a missing `doxygen`. The script keeps the
content of the main page, the namespace page, and each class page, and drops
Doxygen's index pages and page frame. It gives headings their anchors, drops
layout attributes HTML no longer has, and points links at the new URLs, such
as `/using/cpp/api/value-ref/`; a link to a dropped page fails the build. The
ignored `src/generated/cpp-api.json` holds the fragments and their outlines,
`src/pages/using/cpp/api/[...page].astro` places each in a Starlight page, and
the `.iris-cpp-api` rules in `src/styles/custom.css` style Doxygen's markup. The
pages therefore pass the ordinary link, HTML and contrast checks.
`npm run check:cpp-api`, part of `npm run check`, compares the built pages with
that output and requires every public class and the members the guide names.

## Java and Kotlin guide and API references

`/using/java/` renders the examples in `java/iris-examples` the same way,
collected by `scripts/java-examples.mjs`, which also checks that the install
command and the Gradle and Maven snippets still appear in `java/README.md`. The
site build runs no example: `ExamplesTest` in `java/` compiles and runs each one
and compares its output; see the [Java README](../java/README.md#building-and-testing).

Every dev/build runs `scripts/prepare-java-api.mjs`. It runs `:iris:javadoc` and
`:iris-kotlin:dokkaGenerate` through the Gradle wrapper, so a warning from either
tool fails the build, and so does a missing Java 27. The script keeps the content
of the package page and each type page of Javadoc, and of the package page and
each function page of Dokka, and drops the tools' navigation, scripts, styles and
tabs. It turns member signatures into plain anchors, keeps only `class`, `href`
and `id`, and points links at the new URLs, such as `/using/java/api/value-type/`
and `/using/java/kotlin-api/to-int-array/`. Javadoc's serialized form is not
published, so its notes are dropped; any other link to a dropped page fails the
build. Dokka cannot resolve the types of `dev.gecode:iris`, so the script links
them to the Javadoc pages. The ignored `src/generated/java-api.json` and
`kotlin-api.json` hold the fragments and their outlines, the two
`[...page].astro` files in `src/pages/using/java/` place each in a Starlight page,
and the `.iris-java-api` and `.iris-kotlin-api` rules in `src/styles/custom.css`
style the tools' markup. The pages therefore pass the ordinary link, HTML and
contrast checks, with no exclusions. `npm run check:java-api`, part of
`npm run check`, compares the built pages with that output and requires every
public type, every extension, and the members the guide names.

## Playground

`/playground/` runs the real Rust reader in a dedicated module Web Worker.
Edit the reader spec and input, add string parameters, select an instance by
number or name, and inspect Table, JSON, dzn or Summary. Format uses width 100.
The page shows check diagnostics; no lint API exists yet. Every format case and
home showcase has an ordinary Open in playground link, including error cases.

Install the WASM tools explicitly before a normal dev/build:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked --version 0.2.129
```

`npm run prepare:wasm` reads the exact binding version from `iris-wasm`, rejects
CLI mismatches, builds with `--locked` and the `wasm` profile, then optimizes with
the locked Binaryen 132.0.0 dev dependency. It respects `CARGO_TARGET_DIR` and
caller Rust flags. No global tools are installed by scripts. Generated glue,
binary and adapter stay under ignored `src/generated/`; Vite fingerprints the
worker and WASM in `dist/_astro/`. No backend or deployment binding is needed.

`npm run dev:docs` explicitly disables the runtime for documentation-only work
without these tools. Sources remain editable and the page explains the missing
runtime. A normal build restores it; checks reject the disabled adapter.

`npm run check:wasm`, also part of `npm run check`, runs the optimized emitted
binary against every freshly collected format case and compares complete CLI
bytes. It also checks integer/float/Unicode/check/format probes and the share
codec. It never rebuilds or repairs WASM. The hard WASM budget is 1 MiB raw and
384 KiB gzip at level 9; emitted and generated optimized bytes must match.
Binaryen is build-only and is absent from browser code.

`npm run check:browser` uses the test-only Playwright dependency and serves the
existing build on loopback. CI installs managed Chromium; locally use installed
Chrome without a browser download:

```sh
PLAYWRIGHT_MODULE=/Users/zayenz/projects/web/zayenz.se/node_modules/playwright/index.mjs BROWSER_CHANNEL=chrome npm run check:browser
```

Reading is debounced 200 ms. Stop and obsolete work terminate the worker;
operations have a five-second host deadline and startup has fifteen seconds.
Sources are capped at 1 MiB each, origin labels at 256 bytes, parameters at
64 KiB and each complete output at 1 MiB. WASM memory is capped at 256 MiB.
These host limits do not alter Iris work budgets. Table previews mark omissions;
text views keep the exact Rust documents for copy/download. Integer text never
passes through JavaScript numbers.

Copy link stores language, sources, origin labels, exact string parameters and
the selected view in a self-contained `#v1=` gzip/base64url fragment. It does not
include results. Links are limited to 16,384 characters and inflated state to
256 KiB; invalid links leave editors intact. Compression Streams support is
required for sharing, independently of reading. User edits are never fetched
or posted; preset links fetch only the same-origin existing example bundles.
