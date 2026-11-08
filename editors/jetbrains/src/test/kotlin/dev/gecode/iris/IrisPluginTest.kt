package dev.gecode.iris

import com.intellij.codeInsight.daemon.impl.HighlightInfo
import com.intellij.notification.Notification
import com.intellij.notification.Notifications
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.actionSystem.impl.SimpleDataContext
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.EditorFactory
import com.intellij.openapi.editor.event.DocumentEvent
import com.intellij.openapi.editor.event.DocumentListener
import com.intellij.openapi.editor.ex.EditorEx
import com.intellij.openapi.fileTypes.FileTypeManager
import com.intellij.openapi.fileTypes.SyntaxHighlighterFactory
import com.intellij.openapi.util.IconLoader
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServer
import com.intellij.platform.lsp.api.LspServerDescriptor
import com.intellij.platform.lsp.api.LspServerManager
import com.intellij.platform.lsp.api.LspServerManagerListener
import com.intellij.platform.lsp.api.LspServerState
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.testFramework.LightProjectDescriptor
import com.intellij.testFramework.PlatformTestUtil
import com.intellij.testFramework.PsiTestUtil
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import com.intellij.testFramework.fixtures.IdeaTestFixtureFactory
import com.intellij.testFramework.fixtures.TempDirTestFixture
import com.intellij.testFramework.fixtures.impl.CodeInsightTestFixtureImpl
import com.intellij.util.ui.UIUtil
import com.intellij.util.xmlb.XmlSerializer
import org.jetbrains.plugins.textmate.TextMateService
import org.jetbrains.plugins.textmate.language.syntax.highlighting.TextMateHighlighter
import java.awt.GraphicsEnvironment
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import javax.swing.JTextField

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
        // Headless IDEs leave icon loading inactive by default.
        IconLoader.activate()
        try {
            val icon = requireNotNull(file.fileType.icon)
            assertEquals(16, icon.iconWidth)
            assertEquals(16, icon.iconHeight)
            assertTrue(icon.toString().contains("icons/iris.svg"))
        } finally {
            IconLoader.deactivate()
        }
    }

    fun testExecutableLookupAndCommandLine() {
        val directory = File(myFixture.tempDirFixture.tempDirPath, "path with spaces").apply { mkdirs() }
        val name = if (System.getProperty("os.name").startsWith("Windows")) "iris.exe" else "iris"
        val executable = File(directory, name).apply { writeText("test executable"); setExecutable(true) }
        assertEquals(executable.path, findIrisExecutable("", directory.path))
        assertEquals(executable.path, findIrisExecutable(executable.path, ""))
        assertNull(findIrisExecutable(File(directory, "absent").path, directory.path))
        assertNull(findIrisExecutable("", ""))
        val descriptor = IrisLspServerDescriptor(project, executable.path)
        val command = descriptor.createCommandLine()
        assertEquals(executable.path, command.exePath)
        assertEquals(listOf("lsp"), command.parametersList.list)
        assertEquals(project.basePath, command.workDirectory?.path)
        assertTrue(descriptor.isSupportedFile(myFixture.tempDirFixture.createFile("example.iris")))
        assertFalse(descriptor.isSupportedFile(myFixture.tempDirFixture.createFile("example.txt")))
    }

    fun testSettingsPersistenceAndConfigurable() {
        val settings = project.getService(IrisLspSettings::class.java)
        val configurable = IrisLspConfigurable(project)
        try {
            val field = requireNotNull(UIUtil.findComponentOfType(configurable.createComponent(), JTextField::class.java))
            assertFalse(configurable.isModified)
            field.text = "/path with spaces/iris"
            assertTrue(configurable.isModified)
            configurable.apply()
            assertEquals(field.text, settings.executablePath)
            val saved = XmlSerializer.serialize(settings.state)
            val restored = IrisLspSettings()
            restored.loadState(XmlSerializer.deserialize(saved, IrisLspSettings.Options::class.java))
            assertEquals(field.text, restored.executablePath)
            field.text = "discarded"
            configurable.reset()
            assertEquals(settings.executablePath, field.text)
            field.text = ""
            configurable.apply()
            assertEquals("", settings.executablePath)
            assertFalse(configurable.isModified)
        } finally {
            configurable.disposeUIResources()
        }
    }

    fun testMissingExecutableNotification() {
        val received = mutableListOf<Notification>()
        project.messageBus.connect(testRootDisposable).subscribe(Notifications.TOPIC, object : Notifications {
            override fun notify(notification: Notification) {
                if (notification.groupId == "Iris language server") received.add(notification)
            }
        })
        project.getService(IrisLspSettings::class.java).executablePath = ""
        val provider = IrisLspSupportProvider { "" }
        val starter = object : LspServerSupportProvider.LspServerStarter {
            override fun ensureServerStarted(descriptor: LspServerDescriptor) { fail("Missing iris must not start a process") }
        }
        val file = myFixture.tempDirFixture.createFile("missing.iris")
        try {
            provider.fileOpened(project, file, starter)
            PlatformTestUtil.waitWithEventsDispatching("Iris install notification", { received.isNotEmpty() }, 10)
            val notification = received.single()
            assertEquals("Iris executable not found", notification.title)
            assertTrue(notification.content.contains("cargo build --locked -p gecode-iris-cli --bin iris"))
            assertTrue(notification.content.contains("Executable path"))
            assertTrue(notification.content.contains("PATH"))
            assertTrue(notification.actions.any { it.templatePresentation.text == "Configure Iris" })
            provider.fileOpened(project, file, starter)
            UIUtil.dispatchAllInvocationEvents()
            assertEquals("One notification per unresolved attempt", 1, received.size)
        } finally {
            project.getService(IrisLspNotifications::class.java).clear()
        }
    }

    fun testHighlightingDiagnosticAndRestartThroughPlatformClient() {
        // LSP semantic tokens can arrive while doHighlighting dispatches events.
        (myFixture as CodeInsightTestFixtureImpl).canChangeDocumentDuringHighlighting(true)
        val realExecutable = System.getProperty("iris.executable")
        val executable = realExecutable ?: stubExecutable()
        val diagnosticText = if (realExecutable == null) "Iris stub diagnostic" else "not_defined"
        project.getService(IrisLspSettings::class.java).executablePath = executable
        val serverManager = LspServerManager.getInstance(project)
        val diagnosticFile = AtomicReference<VirtualFile>()
        serverManager.addLspServerManagerListener(object : LspServerManagerListener {
            override fun diagnosticsReceived(lspServer: LspServer, file: VirtualFile) {
                if (lspServer.providerClass == IrisLspSupportProvider::class.java) diagnosticFile.set(file)
            }
        }, testRootDisposable)
        val file = myFixture.tempDirFixture.createFile("example.iris", "format \"Example\"\nn: not_defined\n")
        PsiTestUtil.addContentRoot(module, file.parent)
        TextMateService.getInstance().reloadEnabledBundles()
        myFixture.configureFromExistingVirtualFile(file)
        try {
            PlatformTestUtil.waitWithEventsDispatching("Iris initialization", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .any { it.state == LspServerState.Running }
            }, 30)
            PlatformTestUtil.waitWithEventsDispatching("Iris diagnostic received by the platform", {
                diagnosticFile.get() == file
            }, 30)
            PlatformTestUtil.waitWithEventsDispatching("Iris diagnostic in the editor", {
                myFixture.doHighlighting().any { it.description?.contains(diagnosticText) == true }
            }, 30)
            assertTrue("TextMate editor highlighter is applied",
                (myFixture.editor as EditorEx).highlighter.javaClass.name.contains("TextMate"))
            val highlighter = requireNotNull(SyntaxHighlighterFactory.getSyntaxHighlighter(IrisFileType, project, file))
            assertTrue("TextMate highlighter on the Iris file", highlighter is TextMateHighlighter)
            val lexer = highlighter.highlightingLexer
            lexer.start(myFixture.editor.document.charsSequence)
            assertTrue("TextMate assigns attributes to format", highlighter.getTokenHighlights(requireNotNull(lexer.tokenType)).isNotEmpty())
            val firstServer = serverManager.getServersForProvider(IrisLspSupportProvider::class.java).single()
            diagnosticFile.set(null)
            val action = requireNotNull(ActionManager.getInstance().getAction("dev.gecode.iris.RestartLsp"))
            val context = SimpleDataContext.getSimpleContext(CommonDataKeys.PROJECT, project)
            val event = AnActionEvent.createFromAnAction(action, null, "test", context)
            action.actionPerformed(event)
            PlatformTestUtil.waitWithEventsDispatching("Iris replacement server", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .any { it !== firstServer && it.state == LspServerState.Running }
            }, 30)
            PlatformTestUtil.waitWithEventsDispatching("Iris diagnostics after restart", { diagnosticFile.get() == file }, 30)
        } finally {
            serverManager.stopServers(IrisLspSupportProvider::class.java)
            PlatformTestUtil.waitWithEventsDispatching("Iris shutdown", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .none { it.state == LspServerState.Initializing || it.state == LspServerState.Running }
            }, 10)
        }
    }

    fun testDimacsFormatWithRealServer() {
        val executable = requireNotNull(System.getProperty("iris.executable")) {
            "IRIS_TEST_EXECUTABLE must point to a built iris binary"
        }
        assertTrue("The IDE test must run without a display", GraphicsEnvironment.isHeadless())
        // LSP semantic tokens can arrive while doHighlighting dispatches events.
        (myFixture as CodeInsightTestFixtureImpl).canChangeDocumentDuringHighlighting(true)
        project.getService(IrisLspSettings::class.java).executablePath = executable
        val spec = File(requireNotNull(System.getProperty("iris.repository")), "formats/dimacs-cnf/dimacs-cnf.iris")
        val specFile = requireNotNull(LocalFileSystem.getInstance().refreshAndFindFileByIoFile(spec)) {
            "DIMACS reader spec not found: $spec"
        }
        val serverManager = LspServerManager.getInstance(project)
        val publications = AtomicInteger()
        serverManager.addLspServerManagerListener(object : LspServerManagerListener {
            override fun diagnosticsReceived(lspServer: LspServer, file: VirtualFile) {
                if (lspServer.providerClass == IrisLspSupportProvider::class.java && file == specFile) {
                    publications.incrementAndGet()
                }
            }
        }, testRootDisposable)
        PsiTestUtil.addContentRoot(module, specFile.parent)
        TextMateService.getInstance().reloadEnabledBundles()
        myFixture.configureFromExistingVirtualFile(specFile)
        val document = myFixture.editor.document
        val original = document.text
        try {
            PlatformTestUtil.waitWithEventsDispatching("Real iris lsp publishes initial DIMACS diagnostics", {
                publications.get() > 0
            }, 30)
            assertEmpty("The DIMACS reader spec has no diagnostics", diagnosticsInEditor())
            assertTrue("TextMate editor highlighter is applied to DIMACS",
                (myFixture.editor as EditorEx).highlighter.javaClass.name.contains("TextMate"))
            val highlighter = requireNotNull(SyntaxHighlighterFactory.getSyntaxHighlighter(IrisFileType, project, specFile))
            assertTrue("DIMACS uses the TextMate grammar", highlighter is TextMateHighlighter)
            val lexer = highlighter.highlightingLexer
            lexer.start(document.charsSequence)
            assertTrue("TextMate assigns attributes to format",
                highlighter.getTokenHighlights(requireNotNull(lexer.tokenType)).isNotEmpty())

            // The platform sends an incremental didChange from beforeDocumentChange, with the version
            // the document reaches only after every listener has run. A slow listener, as on a loaded
            // CI runner, lets iris lsp publish for that version first; the publish must not be lost.
            // On the editor multicaster, this listener runs after the platform's listener sends didChange.
            val slowBeforeChange = AtomicBoolean(false)
            EditorFactory.getInstance().eventMulticaster.addDocumentListener(object : DocumentListener {
                override fun beforeDocumentChange(event: DocumentEvent) {
                    if (slowBeforeChange.get()) Thread.sleep(SLOW_LISTENER_MILLIS)
                }
            }, testRootDisposable)
            val beforeError = publications.get()
            slowBeforeChange.set(true)
            try {
                WriteCommandAction.runWriteCommandAction(project) {
                    document.insertString(document.textLength, "\ninvalid: not_defined\n")
                }
            } finally {
                slowBeforeChange.set(false)
            }
            PlatformTestUtil.waitWithEventsDispatching("Real iris lsp reports the introduced undefined type", {
                publications.get() > beforeError &&
                    diagnosticsInEditor().any { it.description?.contains("not_defined") == true }
            }, 30)

            val beforeFix = publications.get()
            WriteCommandAction.runWriteCommandAction(project) { document.setText(original) }
            PlatformTestUtil.waitWithEventsDispatching("Real iris lsp clears the diagnostic after the fix", {
                publications.get() > beforeFix && diagnosticsInEditor().isEmpty()
            }, 30)
        } finally {
            // The fixture opens the repository file itself; always restore its document.
            WriteCommandAction.runWriteCommandAction(project) { document.setText(original) }
            serverManager.stopServers(IrisLspSupportProvider::class.java)
            PlatformTestUtil.waitWithEventsDispatching("Real iris lsp shuts down", {
                serverManager.getServersForProvider(IrisLspSupportProvider::class.java)
                    .none { it.state == LspServerState.Initializing || it.state == LspServerState.Running }
            }, 10)
        }
    }

    private companion object {
        // Several times iris lsp's 150 ms diagnostic delay, so the server publishes during the edit.
        const val SLOW_LISTENER_MILLIS = 1_000L
    }

    private fun diagnosticsInEditor(): List<HighlightInfo> =
        myFixture.doHighlighting().filter { it.description != null }

    private fun stubExecutable(): String {
        // Give the executable-path setting a real executable, without changing the production command contract.
        val launcher = File(myFixture.tempDirFixture.tempDirPath, "iris-stub")
        val script = requireNotNull(System.getProperty("iris.stub"))
        launcher.writeText("#!/bin/sh\nexec python3 '${script.replace("'", "'\\''")}' \"\$@\"\n")
        check(launcher.setExecutable(true))
        return launcher.path
    }
}
