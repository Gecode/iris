package org.gecode.iris

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor
import com.intellij.util.EnvironmentUtil
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
}
