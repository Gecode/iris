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
| Rust crates and CLI binaries | `rust-vVERSION` | Released: 0.1.0 on crates.io and GitHub; the crates.io workflow needs the trusted publishing setup below |
| Python | `python-vVERSION` | Ready: the release workflow needs the PyPI setup below |
| TypeScript | `typescript-vVERSION` | Ready: the release workflow needs the npm setup below |
| Java and Kotlin | `java-vVERSION` | Native library and Kotlin module ready; Maven Central publication planned for M7 |
| C++ | `cpp-vVERSION` | Ready: the release workflow needs the GitHub setup below; vcpkg and Conan planned |
| JetBrains plugin | `jetbrains-vVERSION` | Plugin built and verified; Marketplace publication planned for M7 |
| VS Code extension | `vscode-vVERSION` | Not built yet |
| Documentation site | No version tag; deploys from `main` | Deployment configured; production setup and live verification required |

The Rust crates and the Python and TypeScript packages share one version,
because the packages build in the Rust crates from the same commit. Release the
three together: when a change touches Rust code the packages build in, tag
`rust-vVERSION` before or at the same commit as `python-vVERSION` and
`typescript-vVERSION`. The Python and TypeScript workflows refuse a tag when
the Rust sources they build in changed since `rust-vVERSION`. Java and C++
version independently.

Set versions with [scripts/versions.py](scripts/versions.py), which rewrites
every manifest, internal requirement, and lockfile entry, then checks them:

```sh
python3 -I scripts/versions.py set reader 0.1.1   # rust, python and typescript
python3 -I scripts/versions.py set java 0.2.0     # or cpp, or one reader component
python3 -I scripts/versions.py check              # also run by the repository workflow
python3 -I scripts/versions.py release-check python-v0.1.1
```

For the affected component, update its changelog and current installation and
API links together with its versions. Keep historical measurements at their recorded versions. Maintain an
`Unreleased` section in each releasable component's changelog; Rust's is
[rust/CHANGELOG.md](rust/CHANGELOG.md). Move its entries into a versioned section
when preparing that release. Changelog generation from conventional commit
scopes is planned in §9; it is not wired up now.

Use the existing component checks and successful CI as release evidence.
Build and publication commands belong in the component guides below.

## Rust

Follow [rust/README.md](rust/README.md) for development checks, API stability,
MSRV, and publication preparation. The dependency order is `gecode-iris-syntax`, `gecode-iris-sema`,
`gecode-iris-core`, `gecode-iris-gen`, `gecode-iris-macros`, `gecode-iris-build`,
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

From 0.1.1 on, the authorized `rust-vVERSION` tag also starts the
[crates.io workflow](.github/workflows/rust-publish.yml). It runs the release
check, packages and builds every publishable crate with the dry run above, and
after approval in the `crates-io` environment publishes them through trusted
publishing, with no stored token. Set this up once: on crates.io, add a trusted
publisher to each of the ten crates with owner `Gecode`, repository `iris`,
workflow `rust-publish.yml` and environment `crates-io`; on GitHub, create the
`crates-io` environment with a required reviewer. A new crate still needs its
first version published by hand.

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
publishing with no stored token. The PyPI trusted publisher and the `pypi`
environment with its required reviewer are set up.

## TypeScript

An authorized `typescript-vVERSION` tag starts the
[release workflow](.github/workflows/typescript-release.yml). It checks that the
tag names the version in `typescript/package.json`, runs the whole
[TypeScript workflow](.github/workflows/typescript.yml) and publishes the tarball
that workflow packed, with provenance.

npm trusted publishing is configured on a package that already exists, so the
first release used a granular access token, the `NPM_TOKEN` secret of the `npm`
environment. After it, on npmjs.com add a trusted publisher to `gecode-iris` with
owner `Gecode`, repository `iris`, workflow `typescript-release.yml` and
environment `npm`, then delete the secret. npm 11.5.1 and later use trusted
publishing whenever it is configured, so the workflow needs no change; it checks
the npm version.

## C++

An authorized `cpp-vVERSION` tag starts the
[release workflow](.github/workflows/cpp-release.yml). It checks that the tag
names the version in `project(iris VERSION ...)` of `cpp/CMakeLists.txt` and
that [cpp/CHANGELOG.md](cpp/CHANGELOG.md) has a section for that version, runs
the whole [C++ workflow](.github/workflows/cpp.yml), and builds the source
archive `iris-cpp-VERSION.tar.gz` and its checksum
`iris-cpp-VERSION.tar.gz.sha256` with `scripts/cpp-dist.sh`. It then builds the
archive as a project of its own, installs it, and builds a consumer against the
installed package and through FetchContent of the archive by URL and hash. Last,
it creates the GitHub release for the tag with both files, and with the
changelog section as its notes. [cpp/README.md](cpp/README.md#install-and-use)
says what the archive holds and how projects use it.

To prepare a release, set the version in `cpp/CMakeLists.txt`, move the
changelog's `Unreleased` entries into a `## VERSION - DATE` section, and check
the archive on the source commit:

```sh
scripts/cpp-dist.sh VERSION /tmp/iris-dist
scripts/check-cpp-dist.sh /tmp/iris-dist/iris-cpp-VERSION.tar.gz
```

An authorized release then pushes the tag alone:

```sh
git tag -a cpp-vVERSION -m "Iris C++ VERSION" COMMIT
git push origin cpp-vVERSION
```

Set this up once before the first tag: create the `github-release` environment
on GitHub with a required reviewer, so that a tag alone cannot publish. The
release job uses the workflow's own token with `contents: write`, so it needs no
secret. A vcpkg port and a Conan recipe come later.

## Documentation and future packages

Follow [site/PUBLISHING.md](site/PUBLISHING.md) for production setup, deployment
to `iris.gecode.dev`, verification, and rollback. Matching pushes to `main`
deploy automatically once the secrets are configured, so authorization for
such a push must cover the deployment. A configured workflow or a local dry
run does not establish that production is live.

The [Java and Kotlin](java/README.md) and
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
