# Releasing Iris

Every publication needs explicit user authorization, including branch and tag
pushes, registry uploads, GitHub releases, and site deployments. Preparing or
checking a release does not grant that authorization. Confirm the repository,
component, version, and source commit before publishing; push only the intended
tag, never all local tags.

## Versions and preparation

Package versions are independent of language versions and of other components.
[PLAN.md §9](PLAN.md#9-engineering-practices) defines versioning and component
tags; [§11](PLAN.md#11-milestones) defines the release milestones.

| Component | Tag | Current release support |
|---|---|---|
| Rust crates and CLI binaries | `rust-vVERSION` | Released: 0.1.0 on crates.io and GitHub; crates are published by hand until trusted publishing is set up |
| Python | `python-vVERSION` | Ready: the release workflow needs the PyPI setup below |
| TypeScript | `typescript-vVERSION` | Ready: the release workflow needs the npm setup below |
| Java and Kotlin | `java-vVERSION` | Native library and Kotlin module ready; Maven Central publication planned for M7 |
| C++ | `cpp-vVERSION` | Native library and CMake package ready; distribution planned for M7 |
| JetBrains plugin | `jetbrains-vVERSION` | Plugin built and verified; Marketplace publication planned for M7 |
| VS Code extension | `vscode-vVERSION` | Not built yet |
| Documentation site | No version tag; deploys from `main` | Deployment configured; production setup and live verification required |

For the affected component, update its package versions, internal dependency
requirements, lockfiles, changelog, and current installation and API links
together. Keep historical measurements at their recorded versions. Maintain an
`Unreleased` section in each releasable component's changelog; Rust's is
[rust/CHANGELOG.md](rust/CHANGELOG.md). Move its entries into a versioned section
when preparing that release. Changelog generation from conventional commit
scopes is planned in §9; it is not wired up now.

Use the existing component checks and successful CI as release evidence.
Build and publication commands belong in the component guides below.

## Rust

Follow [rust/README.md](rust/README.md) for development checks, API stability,
MSRV, and publication preparation. The dependency order is `gecode-iris-syntax`, `gecode-iris-sema`,
`gecode-iris-core`, `gecode-iris-macros`, `gecode-iris-build`, `gecode-iris-gen`,
`gecode-iris`, `gecode-iris-conformance`, `gecode-iris-lsp`, then `gecode-iris-cli`.
The independent `gecode-iris-conformance` runner can go anywhere before `gecode-iris-cli`.
The M5 browser crate `iris-wasm` stays unpublished and does not enter CLI
archives. If enabled for publication later, it follows `gecode-iris-syntax`,
`gecode-iris-sema` and `gecode-iris-core`. The `iris-driver` conformance driver and
the nightly fuzz crate stay unpublished.

Check a release from `rust/` on a clean checkout of the source commit. This
packages every publishable crate and builds each from its archive, and uploads
nothing:

```sh
cargo publish --dry-run --workspace --locked
```

An authorized publication runs the same command without `--dry-run`, after
`cargo login` with a crates.io token. Cargo publishes in dependency order and
waits for each crate to reach the index. crates.io limits how fast new crate names
can be created, so the first publication can stop partway with a 429 response.
Wait for the time it names, then publish the remaining crates with
`cargo publish --locked -p NAME ...`, as in
[Recover after partial publication](#recover-after-partial-publication).
Registry automation can follow the first publication, because crates.io trusted
publishing is configured on a crate that already exists.

The operational binary path is [Release binaries](rust/README.md#release-binaries):
use its local cargo-dist checks before an authorized `rust-vVERSION` tag push.
That push triggers the [release workflow](.github/workflows/rust-v-release.yml),
which builds and publishes `iris` archives, checksums, and installers to a
GitHub release. [dist-workspace.toml](dist-workspace.toml) owns the target and
installer selection. Homebrew formula generation is enabled; publishing to
the not-yet-created tap is disabled.

## Python

An authorized `python-vVERSION` tag starts the
[release workflow](.github/workflows/python-release.yml). It checks that the tag
names the version in `python/Cargo.toml`, runs the whole
[Python workflow](.github/workflows/python.yml) (lint, every wheel, the source
distribution, conformance) and uploads those exact files to PyPI, through trusted
publishing with no stored token.

Set this up once before the first tag. On PyPI, add a pending trusted publisher
for project `gecode-iris`, owner `Gecode`, repository `iris`, workflow
`python-release.yml` and environment `pypi`. On GitHub, create the `pypi`
environment and add a required reviewer, so that a tag alone cannot publish.

## TypeScript

An authorized `typescript-vVERSION` tag starts the
[release workflow](.github/workflows/typescript-release.yml). It checks that the
tag names the version in `typescript/package.json`, runs the whole
[TypeScript workflow](.github/workflows/typescript.yml) and publishes the tarball
that workflow packed, with provenance.

npm trusted publishing is configured on a package that already exists, so the
first release uses a token. Set this up once before the first tag: create the
`npm` environment on GitHub with a required reviewer, and give it an `NPM_TOKEN`
secret, a granular access token that can publish new packages. After the first
release, configure a trusted publisher for `typescript-release.yml` and delete the
token.

## Documentation and future packages

Follow [site/PUBLISHING.md](site/PUBLISHING.md) for production setup, deployment
to `iris.gecode.dev`, verification, and rollback. Matching pushes to `main`
deploy automatically once the secrets are configured, so authorization for
such a push must cover the deployment. A configured workflow or a local dry
run does not establish that production is live.

The [Java and Kotlin](java/README.md), [C++](cpp/README.md) and
[JetBrains](editors/jetbrains/README.md) guides describe components that have not
been released. Before their first releases, add component changelogs and registry
procedures. Their versions need not match Rust's.

## Recover after partial publication

Check which uploads succeeded before retrying. For crates.io, keep uploaded
versions and resume only the missing crates in dependency order. A correction
to published contents needs a new package version.

For binary releases, inspect the failed workflow and any existing GitHub
release and assets. Retry failed jobs only when doing so preserves already
published artifacts. Do not move a published tag or replace public assets;
use a new version and tag if a source or workflow fix is needed. The current
workflow is tag-triggered and has no manual channel-selection mode.

Recover the site through its [rollback procedure](site/PUBLISHING.md#roll-back).
Retry only the affected component, retain successful publications, and report
the versions and public URLs that succeeded and what remains. Recovery
publications also need explicit user authorization.
