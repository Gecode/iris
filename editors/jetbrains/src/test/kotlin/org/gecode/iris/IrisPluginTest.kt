package org.gecode.iris

import com.intellij.openapi.fileTypes.FileTypeManager
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServer
import com.intellij.platform.lsp.api.LspServerManager
import com.intellij.platform.lsp.api.LspServerManagerListener
import com.intellij.platform.lsp.api.LspServerState
import com.intellij.testFramework.LightProjectDescriptor
import com.intellij.testFramework.PlatformTestUtil
import com.intellij.testFramework.PsiTestUtil
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import com.intellij.testFramework.fixtures.IdeaTestFixtureFactory
import com.intellij.testFramework.fixtures.TempDirTestFixture
import java.util.concurrent.atomic.AtomicReference

class IrisPluginTest : BasePlatformTestCase() {
    // LSP descriptors retain the project directory; each test needs a fresh project.
    private val testProjectDescriptor = LightProjectDescriptor()
    override fun getProjectDescriptor(): LightProjectDescriptor = testProjectDescriptor
    // Server diagnostics use file:// URIs, which cannot resolve the default temp:// fixture.
    override fun createTempDirTestFixture(): TempDirTestFixture =
        IdeaTestFixtureFactory.getFixtureFactory().createTempDirTestFixture()

    fun testFileTypeAndIcon() {
        val file = myFixture.tempDirFixture.createFile("example.iris")
        assertSame(IrisFileType, file.fileType)
        assertSame(IrisFileType, FileTypeManager.getInstance().getFileTypeByExtension("iris"))
        val icon = requireNotNull(file.fileType.icon)
        assertEquals(16, icon.iconWidth)
        assertEquals(16, icon.iconHeight)
        assertTrue(icon.toString().contains("icons/iris.svg"))
    }

    fun testConfiguredCommandLine() {
        val settings = project.getService(IrisLspSettings::class.java)
        settings.command = "\"/path with spaces/iris\" lsp --stdio"
        val descriptor = IrisLspServerDescriptor(project)
        val command = descriptor.createCommandLine()
        assertEquals("/path with spaces/iris", command.exePath)
        assertEquals(listOf("lsp", "--stdio"), command.parametersList.list)
        assertEquals(project.basePath, command.workDirectory?.path)
        val file = myFixture.tempDirFixture.createFile("example.iris")
        assertTrue(descriptor.isSupportedFile(file))
        val other = myFixture.tempDirFixture.createFile("example.txt")
        assertFalse(descriptor.isSupportedFile(other))
    }

    fun testStubDiagnosticThroughPlatformClient() {
        project.getService(IrisLspSettings::class.java).command = "python3 \"${System.getProperty("iris.stub")}\""
        val serverManager = LspServerManager.getInstance(project)
        val diagnosticFile = AtomicReference<VirtualFile>()
        serverManager.addLspServerManagerListener(object : LspServerManagerListener {
            override fun diagnosticsReceived(lspServer: LspServer, file: VirtualFile) {
                if (lspServer.providerClass == IrisLspSupportProvider::class.java) diagnosticFile.set(file)
            }
        }, testRootDisposable)
        val file = myFixture.tempDirFixture.createFile(
            "example.iris", requireNotNull(javaClass.getResource("/example.iris")).readText()
        )
        PsiTestUtil.addContentRoot(module, file.parent)
        myFixture.configureFromExistingVirtualFile(file)
        try {
            PlatformTestUtil.waitWithEventsDispatching("Iris stub initialization", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .any { it.initializeResult?.serverInfo?.name == "iris-stub" }
            }, 30)
            PlatformTestUtil.waitWithEventsDispatching("Iris stub diagnostic received by the platform", {
                diagnosticFile.get() == myFixture.file.virtualFile
            }, 30)
            PlatformTestUtil.waitWithEventsDispatching("Iris stub diagnostic in the editor", {
                myFixture.doHighlighting().any { it.description?.contains("Iris stub diagnostic") == true }
            }, 30)
        } finally {
            serverManager.stopServers(IrisLspSupportProvider::class.java)
            PlatformTestUtil.waitWithEventsDispatching("Iris stub shutdown", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .none { it.state == LspServerState.Initializing || it.state == LspServerState.Running }
            }, 10)
        }
    }
}
