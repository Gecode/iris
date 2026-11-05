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
| Rust crates and CLI binaries | `rust-vVERSION` | cargo-dist workflow ready; crates.io publication planned for M7 |
| Python | `python-vVERSION` | Planned: binding, wheels, source distribution, and registry publication |
| Java and Kotlin | `java-vVERSION` | Native library and Kotlin module ready; Maven Central publication planned for M7 |
| C++ | `cpp-vVERSION` | Planned: native library, CMake package, and distribution |
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
MSRV, and publication preparation. All workspace crates currently have
`publish = false`; enabling crates.io publication and registry automation is
M7 work. The dependency order is `iris-syntax`, `iris-sema`, `iris-core`,
`iris-gen`, `gecode-iris`, `iris-conformance`, `iris-lsp`, then `iris-cli`.
The independent `iris-conformance` runner can go anywhere before `iris-cli`.
The M5 browser crate `iris-wasm` stays unpublished and does not enter CLI
archives. If enabled for publication later, it follows `iris-syntax`,
`iris-sema` and `iris-core`. The nightly fuzz crate stays unpublished.

The operational binary path is [Release binaries](rust/README.md#release-binaries):
use its local cargo-dist checks before an authorized `rust-vVERSION` tag push.
That push triggers the [release workflow](.github/workflows/rust-v-release.yml),
which builds and publishes `iris` archives, checksums, and installers to a
GitHub release. [dist-workspace.toml](dist-workspace.toml) owns the target and
installer selection. Homebrew formula generation is enabled; publishing to
the not-yet-created tap is disabled.

## Documentation and future packages

Follow [site/PUBLISHING.md](site/PUBLISHING.md) for production setup, deployment
to `iris.gecode.dev`, verification, and rollback. Matching pushes to `main`
deploy automatically once the secrets are configured, so authorization for
such a push must cover the deployment. A configured workflow or a local dry
run does not establish that production is live.

The [Python](python/README.md), [Java and Kotlin](java/README.md), and
[C++](cpp/README.md) guides describe planned components. Before their first
releases, add component changelogs and registry procedures.
Python's procedure must cover checked wheels and a source distribution that
builds without a sibling checkout. Their versions need not match Rust's.

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
