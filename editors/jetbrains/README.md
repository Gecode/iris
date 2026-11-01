# Iris for JetBrains IDEs

This plugin implements [PLAN.md §8.14](../../PLAN.md#814-jetbrains-plugin-editorsjetbrains),
part of milestone M4b. It registers `.iris` reader specs and their icon,
highlights them with the shared TextMate grammar, and starts `iris lsp`
through the IntelliJ Platform LSP API. The plugin supplies no native
parser or PSI implementation.

## Build

Run in this directory:

```sh
./gradlew build verifyPlugin
```

The checked-in Gradle 9.8.2 wrapper verifies its distribution checksum.
Gradle can run on the installed Java 27; the Foojay toolchain resolver
downloads Java 21 for compilation and tests. The plugin targets the IDE's
Java 21 runtime, independently of the Iris Java library's Java 27 baseline.
The default integration test needs a Unix shell and Python 3.10+ on `PATH`
for the stub. The first build needs network access and several GiB for
IDEs and caches.

The build uses Kotlin 2.2.20 and IntelliJ Platform Gradle Plugin 2.19.0,
compiling against IntelliJ IDEA 2025.3.3. The descriptor accepts builds
253–262 (2025.3, 2026.1, and 2026.2), covering the past-year major release
lines as of 2026-11-01. Intended products are IntelliJ IDEA, CLion,
PyCharm, RustRover, and other IDEs providing `com.intellij.modules.lsp`
and the bundled `org.jetbrains.plugins.textmate` plugin. Android Studio
and open-source IDEA builds lack the LSP module.

The plugin deliberately compiles against the older, server-named API
(`LspServerSupportProvider`, `LspServerManager`, and
`ProjectWideLspServerDescriptor`). The client-named API introduced in
2026.1.4 is unavailable in 2025.3. Newer IDEs retain the old API as
deprecated compatibility shims; no reflection or per-version adapter is
needed. Rename needs IDE 2026.1.1 and range formatting needs 2026.1.

`verifyPlugin` checks representative IDEA endpoints, 2025.3.3 and 2026.2.4,
including resolution of the bundled TextMate dependency. This is not the
full product/release matrix planned for CI. Verifier reports are in
`build/reports/pluginVerifier/`; JUnit reports are in
`build/reports/tests/test/`. The installable ZIP is
`build/distributions/iris-0.1.0-SNAPSHOT.zip`. Build output, Gradle and
Kotlin caches, and the sandbox under `build/sandbox/` are ignored by Git.

## Shared grammar and artwork

`prepareTextMateBundle` copies
[`editors/textmate/iris.tmLanguage.json`](../textmate/iris.tmLanguage.json)
and the small bundle manifest in `src/main/textmate/package.json` into
`build/textmate/iris/`. Every sandbox preparation task copies that directory
into the plugin's `textmate/iris/` directory, which also goes into the ZIP.
There is one tracked grammar. The platform TextMate bundle provider reads
this directory; the Iris file type delegates syntax and editor highlighting
to the bundled TextMate implementation.

The resource task copies the tuned mark from
[`brand/favicon/favicon.svg`](../../brand/favicon/favicon.svg), setting
only the SVG display size: 16 pixels for the file type, 40 for the plugin.
Kotlin's standard library comes from the IDE.

## Server settings and restart

In **Settings → Languages & Frameworks → Iris**, set **Executable path**
to the absolute path of the `iris` binary, for example
`/home/me/iris/rust/target/debug/iris`. Paths containing spaces need no
quotes. Enter only the path: the plugin adds the `lsp` argument and starts
the process directly. Shell expressions and command-line arguments do not
belong in this field.

An empty field searches for `iris` (`iris.exe` on Windows) on the IDE's
console `PATH`. A configured path takes precedence; an unusable configured
file produces the install notification rather than silently selecting a
different binary. The process inherits the console environment and uses
the project directory as its working directory. Settings are stored in
the project's workspace settings.

Applying a changed setting restarts the server for open reader specs.
**Find Action → Restart Iris Language Server** also stops and restarts it,
using the latest setting and `PATH`. The platform owns the server process
and reopens documents after initialization.

When no usable binary is found, a warning explains how to build `iris`:

```sh
# In rust/ at the root of this repository:
cargo build --locked -p iris-cli --bin iris
```

The notification names `rust/target/debug/iris` (`iris.exe` on Windows),
the executable-path setting, the `PATH` fallback, and the restart action.
**Configure Iris** opens the settings page. Repeated file opens share one
notification until it expires or lookup succeeds. The plugin does
not download a server; the release download offer belongs to jetbrains-004.
See [`iris-lsp`](../../rust/iris-lsp/README.md) for server behavior.

## Automated checks

Five headless IDE tests cover file recognition and the 16-pixel icon,
configured-path precedence and `PATH` lookup, command construction with
spaces, settings UI apply/reset and XML persistence, and the missing-binary
notification with an injected empty search path. The integration test opens
a reader spec with an unresolved type, asserts a TextMate highlighter and
non-default token attributes, receives a server diagnostic in the platform
and editor, invokes the registered restart action, and receives a diagnostic
from the replacement session. Both sessions shut down normally.

By default the integration test uses `src/test/resources/stub_server.py`
through a temporary executable launcher. The stub is absent from the ZIP;
its diagnostic checks transport and editor integration, not Iris semantics.
To run the same integration test against the real server, build with a
separate target directory and pass the executable:

```sh
(cd ../../rust && CARGO_TARGET_DIR=/tmp/iris-jetbrains-test-target cargo build --locked -p iris-cli --bin iris)
IRIS_TEST_EXECUTABLE=/tmp/iris-jetbrains-test-target/debug/iris ./gradlew test
```

The test IDE log is
`build/sandbox/iris/IU-2025.3.3/log-test/idea.log`. Its language-service logs
record initialization, document diagnostics, restart, and shutdown.

## Validation evidence (2026-11-01)

`./gradlew build verifyPlugin` passed with five tests, zero failures, errors,
or skips, using the stub. Plugin Verifier 1.411 reported IDEA 2025.3.3
(build `253.31033.145`) compatible and IDEA 2026.2.4 (build `262.11879.100`)
compatible with 13 deprecated LSP API usages. The bundled TextMate dependency
resolved on both endpoints; no internal API usages were reported.
`scripts/check-repo.sh` also passed.

A brief `runIde` check logged application startup, loading Iris, and opening
an ignored stub project, then shutdown after SIGTERM. It did not establish
a sandbox LSP session; session evidence comes from the headless test. No GUI
inspection was performed.

## Sandbox and remaining checks

```sh
./gradlew runIde
```

Set the executable path, open a reader spec with an error, and use
**Restart Iris Language Server** to retry after an installation or path
change. The sandbox IDE log is
`build/sandbox/iris/IU-2025.3.3/log_runIde/idea.log`.

Manual visual checks are waived for unattended jetbrains-003 work.
jetbrains-006 still needs IntelliJ IDEA and CLion checks of the visible
highlighting, icon, warning underline, Problems view and diagnostic code
links, plus the §8.7 feature walkthrough. Headless token and diagnostic
assertions do not establish their visual presentation. The other target
products and release lines also need the planned compatibility matrix.
