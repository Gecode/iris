package dev.gecode.iris

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.application.ModalityState
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.Lsp4jClient
import com.intellij.platform.lsp.api.LspServerNotificationsHandler
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor
import com.intellij.util.EnvironmentUtil
import org.eclipse.lsp4j.PublishDiagnosticsParams
import java.io.File

// The server-named API also works on 2025.3, before the 2026.1.4 client API rename.
class IrisLspSupportProvider internal constructor(
    private val release: IrisBinaryDownload,
    private val searchPath: () -> String?,
) : LspServerSupportProvider {
    constructor() : this(IrisBinaryDownload(), { EnvironmentUtil.getValue("PATH") })
    internal constructor(searchPath: () -> String?) : this(IrisBinaryDownload(), searchPath)

    override fun fileOpened(project: Project, file: VirtualFile, serverStarter: LspServerSupportProvider.LspServerStarter) {
        if (file.fileType != IrisFileType) return
        val configuredPath = project.getService(IrisLspSettings::class.java).executablePath
        val executable = findIrisExecutable(configuredPath, searchPath())
            ?: if (configuredPath.isBlank()) release.cachedExecutable() else null
        val notifications = project.getService(IrisLspNotifications::class.java)
        if (executable == null) {
            notifications.showMissingExecutable(if (configuredPath.isBlank()) release else null)
            return
        }
        notifications.clear()
        serverStarter.ensureServerStarted(IrisLspServerDescriptor(project, executable))
    }
}

internal fun findIrisExecutable(configuredPath: String, searchPath: String?): String? {
    fun executable(file: File): String? = file.takeIf { it.isFile && it.canExecute() }?.absolutePath
    if (configuredPath.isNotBlank()) return executable(File(configuredPath))
    val name = if (System.getProperty("os.name").startsWith("Windows")) "iris.exe" else "iris"
    return searchPath?.split(File.pathSeparatorChar)?.asSequence()
        ?.filter { it.isNotBlank() }
        ?.mapNotNull { executable(File(it, name)) }
        ?.firstOrNull()
}

class IrisLspServerDescriptor(project: Project, private val executable: String) : ProjectWideLspServerDescriptor(project, "Iris") {
    override fun isSupportedFile(file: VirtualFile): Boolean = file.fileType == IrisFileType

    override fun createCommandLine(): GeneralCommandLine =
        GeneralCommandLine(executable, "lsp")
            .withWorkDirectory(project.basePath)
            .withParentEnvironmentType(GeneralCommandLine.ParentEnvironmentType.CONSOLE)

    override fun createLsp4jClient(handler: LspServerNotificationsHandler): Lsp4jClient =
        IrisLsp4jClient(DiagnosticsAfterEdits(handler, project))
}

// The platform API constructs its client only through a subclass.
private class IrisLsp4jClient(handler: LspServerNotificationsHandler) : Lsp4jClient(handler)

/**
 * Applies published diagnostics after any edit in progress on the EDT.
 *
 * The platform sends an incremental didChange from `beforeDocumentChange`, with the version the
 * document reaches only after every other listener has run, and it discards a publish whose version
 * differs from the document's. When those listeners take longer than iris lsp's diagnostic delay,
 * the diagnostics for the edit arrive first and are lost until the next edit. The EDT queue runs
 * this publish after that edit's write action, when the document has the published version.
 */
private class DiagnosticsAfterEdits(
    private val handler: LspServerNotificationsHandler,
    private val project: Project,
) : LspServerNotificationsHandler by handler {
    override fun publishDiagnostics(params: PublishDiagnosticsParams) {
        ApplicationManager.getApplication().invokeLater(
            { handler.publishDiagnostics(params) }, ModalityState.any(), project.disposed)
    }
}
