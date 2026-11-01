# Iris for JetBrains IDEs

This is the minimal plugin spike for [PLAN.md §8.14](../../PLAN.md#814-jetbrains-plugin-editorsjetbrains),
part of milestone M4b. It registers `.iris` reader specs and starts a
configurable command through the IntelliJ Platform LSP API. The LSP client
and server acquisition choices remain open in [PLAN.md §12](../../PLAN.md#12-decisions).
TextMate highlighting, a restart action, and server installation are later
tasks; this spike does not bundle the grammar or a server.

## Build

Run in this directory:

```sh
./gradlew build verifyPlugin
```

The checked-in Gradle 9.8.2 wrapper verifies its distribution checksum.
Gradle can run on the installed Java 27; the Foojay toolchain resolver
downloads Java 21 for compilation and tests. The plugin targets the IDE's
Java 21 runtime, independently of the Iris Java library's Java 27 baseline.
Tests need Python 3.10+ on `PATH` for the stub. The first build needs network
access and several GiB for IDEs and caches.

The build uses Kotlin 2.2.20 and IntelliJ Platform Gradle Plugin 2.19.0,
compiling against IntelliJ IDEA 2025.3.3. The descriptor accepts builds
253–262 (2025.3, 2026.1, and 2026.2), covering the past-year major release
lines as of 2026-11-01. Intended products are IntelliJ IDEA, CLion,
PyCharm, RustRover, and other IDEs providing `com.intellij.modules.lsp`.
Android Studio and open-source IDEA builds lack that module.

`verifyPlugin` checks representative IDEA endpoints, 2025.3.3 and 2026.2.4.
This is not the full product/release matrix planned for CI. The verifier
reports are in `build/reports/pluginVerifier/`; JUnit reports are in
`build/reports/tests/test/`. The installable ZIP is
`build/distributions/iris-0.1.0-SNAPSHOT.zip`. Build output, Gradle caches,
and the sandbox under `build/sandbox/` are ignored by Git.

The resource task copies the tuned mark from
[`brand/favicon/favicon.svg`](../../brand/favicon/favicon.svg), setting
only the SVG display size: 16 pixels for the file type, 40 for the plugin.
No artwork is redrawn. Kotlin's standard library comes from the IDE.
The file type is registered on virtual files; the plugin supplies no native
parser or PSI implementation.

## Server configuration

In **Settings → Languages & Frameworks → Iris**, enter a command such as
`iris lsp` or `"/path with spaces/iris" lsp`. The project stores the command
in its workspace settings. Arguments use IntelliJ's command-line quoting;
shell syntax, variable expansion, and pipelines are not supported. The
process inherits the console environment and uses the project directory
as its working directory. Reopen the project after changing the command.

## Sandbox and manual check

```sh
./gradlew runIde
```

In the sandbox IDE:

1. Complete any first-launch prompts, then open `src/test/resources/` in
   this plugin directory as a project. Trust this local test project if asked.
2. In **Settings → Languages & Frameworks → Iris**, set the command to
   `python3 "/absolute/path/to/iris/editors/jetbrains/src/test/resources/stub_server.py"`.
   Use an absolute Python path if it is absent from the IDE environment.
3. Close and reopen that project, then open `example.iris`.
4. Check the coloured Iris mark beside the file in the project tree and
   tab. **Settings → Editor → File Types** should list Iris reader spec
   with `*.iris`.
5. The first `format` should have a warning underline. Hover it or open
   **View → Tool Windows → Problems**: expect **Iris stub diagnostic**,
   from `iris-stub`. Check whether the `iris-stub` code links to the
   Gecode website; code-link rendering remains an open evidence item.

The Python stub answers `initialize`, publishes one warning on document
open/change, and handles shutdown/exit. It lives only in test resources
and is absent from the plugin ZIP. Its diagnostic deliberately says
nothing about whether the reader spec is valid. Highlighting beyond that
diagnostic is not part of this spike.

## Validation evidence (2026-11-01)

`./gradlew build verifyPlugin` reported `BUILD SUCCESSFUL in 11s`.
JUnit reported three tests, zero failures, zero errors, and zero skips.
The tests cover file recognition and the 16-pixel icon resource, plus the
configured executable, quoted arguments, working directory, and file
selection. The integration test opens the sample in a fresh project with
files on disk, starts the configured stub through the registered provider,
waits for `iris-stub` initialization and the platform's diagnostic event,
and finds **Iris stub diagnostic** in editor highlighting. The server
then shuts down normally with exit code 0. Real files are necessary so
the server's `file://` URIs resolve; the default `temp://` fixture does
not round-trip through stdio LSP.

Plugin Verifier 1.411 reported:

```text
IU-253.31033.145 (2025.3.3): Compatible
IU-262.11879.100 (2026.2.4): Compatible. 7 usages of deprecated API
```

Those deprecations are the retained server-named LSP API used for 2025.3
compatibility. The build also reports Gradle deprecations for Gradle 10
and IDE layout warnings for bundled plugins; neither is a compatibility
failure for this plugin. Full output is in `build/validation.log`.
The test IDE log is `build/sandbox/iris/IU-2025.3.3/log-test/idea.log`.

`./gradlew runIde --args="$PWD/build/stub-project"` launched IDEA 2025.3.3
against a prepared, ignored copy of the sample project. Its log at
`build/sandbox/iris/IU-2025.3.3/log_runIde/idea.log` recorded `IDE STARTED`,
`Loaded custom plugins: Iris (0.1.0-SNAPSHOT)`, and an attempt to open that
project. After about a minute the IDE was stopped with SIGTERM; the log
recorded `IDE SHUTDOWN`, and Gradle reported exit 143 as expected.
The sandbox log did not establish an active project LSP session.

No GUI inspection was performed. The headless test establishes diagnostic
reception and editor highlighting data; the sandbox check establishes
application startup and plugin loading. The manual steps above remain
necessary to confirm the visible file-type icon, warning underline,
Problems view, and diagnostic code link. CLion and the other target IDEs
still need their planned product/release matrix and visual checks.
