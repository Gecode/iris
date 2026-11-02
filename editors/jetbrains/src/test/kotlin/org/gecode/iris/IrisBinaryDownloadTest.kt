package org.gecode.iris

import com.intellij.notification.Notification
import com.intellij.notification.Notifications
import com.intellij.openapi.actionSystem.impl.SimpleDataContext
import com.intellij.platform.lsp.api.LspServerDescriptor
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.testFramework.LightProjectDescriptor
import com.intellij.testFramework.PlatformTestUtil
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import com.intellij.testFramework.fixtures.IdeaTestFixtureFactory
import com.intellij.testFramework.fixtures.TempDirTestFixture
import com.intellij.util.io.Compressor
import com.intellij.util.ui.UIUtil
import com.sun.net.httpserver.HttpServer
import java.net.InetSocketAddress
import java.nio.file.Files
import java.nio.file.Path
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ConcurrentLinkedQueue

class IrisBinaryDownloadTest : BasePlatformTestCase() {
    private val testProjectDescriptor = LightProjectDescriptor()
    override fun getProjectDescriptor(): LightProjectDescriptor = testProjectDescriptor
    override fun createTempDirTestFixture(): TempDirTestFixture =
        IdeaTestFixtureFactory.getFixtureFactory().createTempDirTestFixture()
    private lateinit var server: HttpServer
    private lateinit var directory: Path
    private val artifacts = ConcurrentHashMap<String, ByteArray>()
    private val requests = ConcurrentLinkedQueue<String>()
    private val notifications = mutableListOf<Notification>()

    override fun setUp() {
        super.setUp()
        directory = Path.of(myFixture.tempDirFixture.tempDirPath)
        server = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        server.createContext("/") { exchange ->
            val path = exchange.requestURI.path
            requests.add(path)
            val bytes = artifacts[path]
            exchange.sendResponseHeaders(if (bytes == null) 404 else 200, bytes?.size?.toLong() ?: -1)
            exchange.responseBody.use { if (bytes != null) it.write(bytes) }
            exchange.close()
        }
        server.start()
        project.messageBus.connect(testRootDisposable).subscribe(Notifications.TOPIC, object : Notifications {
            override fun notify(notification: Notification) {
                if (notification.groupId == "Iris language server") notifications.add(notification)
            }
        })
    }

    override fun tearDown() {
        try {
            project.getService(IrisLspNotifications::class.java).clear()
            notifications.forEach { it.expire() }
            server.stop(0)
        } finally {
            super.tearDown()
        }
    }

    fun testAcceptDownloadAndReuseCachedBinary() {
        val release = releaseFixture()
        val provider = IrisLspSupportProvider(release) { "" }
        assertNull(openSpec(provider))
        val offer = missingNotification()
        assertTrue(offer.content.contains("0.1.0"))
        assertTrue(offer.content.contains(requireNotNull(release.target)))
        assertTrue(offer.content.contains(release.releaseUrl))
        assertEmpty("Opening a reader spec does not start a download", requests)
        invoke(offer, "Download iris 0.1.0")
        PlatformTestUtil.waitWithEventsDispatching("Accepted Iris download completes", { offer.isExpired }, 20)
        val executable = requireNotNull(release.cachedExecutable())
        assertEquals(executable, openSpec(provider)?.createCommandLine()?.exePath)
        assertEquals(listOf("lsp"), openSpec(provider)?.createCommandLine()?.parametersList?.list)
        assertEquals(listOf(
            "/rust-v0.1.0/${release.asset}.sha256",
            "/rust-v0.1.0/${release.asset}",
        ), requests.toList())
        val installation = Path.of(executable).parent
        assertEquals("fixture license", Files.readString(installation.resolve("LICENSE")))
        assertEquals(setOf("iris", "LICENSE", "README.md"), Files.list(installation).use { paths ->
            paths.map { it.fileName.toString() }.toList().toSet()
        })
        assertNoStagingFiles()
    }

    fun testWindowsZipLayout() {
        val release = releaseFixture(os = "Windows 11", arch = "amd64")
        assertEquals("rust-x86_64-pc-windows-msvc.zip", release.asset)
        assertNull(openSpec(IrisLspSupportProvider(release) { "" }))
        val offer = missingNotification()
        invoke(offer, "Download iris 0.1.0")
        PlatformTestUtil.waitWithEventsDispatching("Windows archive installed", { offer.isExpired }, 20)
        assertTrue(requireNotNull(release.cachedExecutable()).endsWith("iris.exe"))
        assertNoStagingFiles()
    }

    fun testDeclineKeepsMissingNotification() {
        val release = releaseFixture()
        val provider = IrisLspSupportProvider(release) { "" }
        assertNull(openSpec(provider))
        val offer = missingNotification()
        invoke(offer, "Not now")
        assertNull(openSpec(provider))
        UIUtil.dispatchAllInvocationEvents()
        assertFalse(offer.isExpired)
        assertEquals(listOf(offer), notifications)
        assertTrue(offer.content.contains("cargo build --locked -p iris-cli --bin iris"))
        assertEmpty(requests)
        assertNull(release.cachedExecutable())
    }

    fun testChecksumMismatchRefusesAndDiscardsDownload() {
        val release = releaseFixture(checksum = "0".repeat(64))
        assertNull(openSpec(IrisLspSupportProvider(release) { "" }))
        val offer = missingNotification()
        invoke(offer, "Download iris 0.1.0")
        val failure = downloadFailure()
        assertTrue(failure.content.contains("SHA-256 mismatch"))
        assertFalse(offer.isExpired)
        assertNull(release.cachedExecutable())
        assertNoInstalledFiles()
    }

    fun testChecksumMustNameTheExactArchive() {
        val release = releaseFixture()
        artifacts["/rust-v0.1.0/${release.asset}.sha256"] =
            ("0".repeat(64) + " *some-other-archive.tar.gz\n").toByteArray()
        assertNull(openSpec(IrisLspSupportProvider(release) { "" }))
        invoke(missingNotification(), "Download iris 0.1.0")
        assertTrue(downloadFailure().content.contains("Invalid SHA-256 checksum file"))
        assertNoInstalledFiles()
    }

    fun testWrongServerVersionIsNotInstalled() {
        val release = releaseFixture(serverVersion = "0.2.0")
        assertNull(openSpec(IrisLspSupportProvider(release) { "" }))
        invoke(missingNotification(), "Download iris 0.1.0")
        assertTrue(downloadFailure().content.contains("did not report version 0.1.0"))
        assertNoInstalledFiles()
    }

    fun testUnsupportedPlatformAndReleaseTargets() {
        val release = releaseFixture(os = "Windows 11", arch = "aarch64")
        assertNull(openSpec(IrisLspSupportProvider(release) { "" }))
        val notification = missingNotification()
        assertTrue(notification.content.contains("No Iris release is available for Windows 11 (aarch64)"))
        assertEquals(listOf("Configure Iris"), notification.actions.map { it.templatePresentation.text })
        assertEmpty(requests)
        assertNull(irisReleaseTarget("FreeBSD", "amd64"))
        assertNull(irisReleaseTarget("Linux", "riscv64"))
        assertEquals("x86_64-unknown-linux-gnu", irisReleaseTarget("Linux", "amd64"))
        assertEquals("aarch64-unknown-linux-gnu", irisReleaseTarget("Linux", "aarch64"))
        assertEquals("x86_64-apple-darwin", irisReleaseTarget("Mac OS X", "x86_64"))
        assertEquals("aarch64-apple-darwin", irisReleaseTarget("Mac OS X", "arm64"))
        assertEquals("x86_64-pc-windows-msvc", irisReleaseTarget("Windows 11", "x86_64"))
    }

    fun testConfiguredPathAndPathTakePrecedence() {
        val release = releaseFixture()
        val pathDirectory = Files.createDirectory(directory.resolve("path"))
        val onPath = executable(pathDirectory.resolve("iris"))
        val configured = executable(directory.resolve("configured-iris"))
        val provider = IrisLspSupportProvider(release) { pathDirectory.toString() }
        val settings = project.getService(IrisLspSettings::class.java)
        settings.executablePath = configured.toString()
        assertEquals(configured.toString(), openSpec(provider)?.createCommandLine()?.exePath)
        settings.executablePath = ""
        assertEquals(onPath.toString(), openSpec(provider)?.createCommandLine()?.exePath)
        assertEmpty(requests)
        assertEmpty(notifications)

        // Even with an installed download, explicit configuration and PATH win.
        val cached = executable(directory.resolve("cache/0.1.0/${release.target}/iris"))
        assertEquals(cached.toString(), release.cachedExecutable())
        assertEquals(onPath.toString(), openSpec(provider)?.createCommandLine()?.exePath)
        settings.executablePath = configured.toString()
        assertEquals(configured.toString(), openSpec(provider)?.createCommandLine()?.exePath)
        settings.executablePath = directory.resolve("absent").toString()
        assertNull(openSpec(provider))
        assertEquals(listOf("Configure Iris"), missingNotification().actions.map { it.templatePresentation.text })
        assertEmpty(requests)
    }

    private fun openSpec(provider: IrisLspSupportProvider): LspServerDescriptor? {
        var started: LspServerDescriptor? = null
        val starter = object : LspServerSupportProvider.LspServerStarter {
            override fun ensureServerStarted(descriptor: LspServerDescriptor) { started = descriptor }
        }
        provider.fileOpened(project, myFixture.tempDirFixture.createFile("download.iris"), starter)
        return started
    }

    private fun missingNotification(): Notification {
        PlatformTestUtil.waitWithEventsDispatching("Iris download offer", { notifications.isNotEmpty() }, 10)
        return notifications.single { it.title == "Iris executable not found" }
    }

    private fun invoke(notification: Notification, text: String) {
        val action = notification.actions.single { it.templatePresentation.text == text }
        val context = SimpleDataContext.getSimpleContext(Notification.KEY, notification, SimpleDataContext.getProjectContext(project))
        Notification.fire(notification, action, context)
    }

    private fun downloadFailure(): Notification {
        PlatformTestUtil.waitWithEventsDispatching("Iris download failure", {
            notifications.any { it.title == "Iris download failed" }
        }, 20)
        return notifications.single { it.title == "Iris download failed" }
    }

    private fun assertNoStagingFiles() {
        Files.walk(directory.resolve("cache")).use { paths ->
            assertFalse(paths.anyMatch { it.fileName.toString().startsWith(".download-") })
        }
    }

    private fun assertNoInstalledFiles() {
        Files.walk(directory.resolve("cache")).use { paths -> assertFalse(paths.anyMatch { Files.isRegularFile(it) }) }
        assertNoStagingFiles()
    }

    private fun executable(path: Path, version: String = "0.1.0"): Path {
        Files.createDirectories(path.parent)
        // A real executable exercises the version probe; LSP transport is covered by IrisPluginTest.
        Files.writeString(path, "#!/bin/sh\nprintf 'iris $version (languages: 0.1)\\n'\n")
        check(path.toFile().setExecutable(true))
        return path
    }

    private fun releaseFixture(
        os: String = System.getProperty("os.name"),
        arch: String = System.getProperty("os.arch"),
        checksum: String? = null,
        serverVersion: String = "0.1.0",
    ): IrisBinaryDownload {
        val release = IrisBinaryDownload(
            version = "0.1.0", os = os, arch = arch,
            baseUrl = "http://127.0.0.1:${server.address.port}", cacheRoot = directory.resolve("cache"),
        )
        val asset = release.asset ?: return release
        val archive = directory.resolve(asset)
        val isZip = asset.endsWith(".zip")
        val binaryName = if (isZip) "iris.exe" else "iris"
        val prefix = if (isZip) "" else "rust-${release.target}/"
        val binary = executable(directory.resolve("fixture-$binaryName"), serverVersion)
        val compressor = if (isZip) Compressor.Zip(archive) else Compressor.Tar(archive, Compressor.Tar.Compression.GZIP)
        compressor.use {
            it.addFile("$prefix$binaryName", binary)
            it.addFile("${prefix}LICENSE", "fixture license".toByteArray())
            it.addFile("${prefix}README.md", "fixture readme".toByteArray())
            it.addFile("${prefix}unrelated", "must not be extracted".toByteArray())
        }
        val bytes = Files.readAllBytes(archive)
        val sha256 = checksum ?: MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        artifacts["/rust-v0.1.0/$asset"] = bytes
        artifacts["/rust-v0.1.0/$asset.sha256"] = "$sha256 *$asset\n\n".toByteArray()
        return release
    }
}
