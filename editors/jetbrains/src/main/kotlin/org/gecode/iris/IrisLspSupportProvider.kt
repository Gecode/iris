package org.gecode.iris

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor
import com.intellij.util.execution.ParametersListUtil

// The server-named API also works on 2025.3, before the 2026.1.4 client API rename.
class IrisLspSupportProvider : LspServerSupportProvider {
    override fun fileOpened(project: Project, file: VirtualFile, serverStarter: LspServerSupportProvider.LspServerStarter) {
        if (file.fileType == IrisFileType) {
            serverStarter.ensureServerStarted(IrisLspServerDescriptor(project))
        }
    }
}

class IrisLspServerDescriptor(project: Project) : ProjectWideLspServerDescriptor(project, "Iris") {
    override fun isSupportedFile(file: VirtualFile): Boolean = file.fileType == IrisFileType

    override fun createCommandLine(): GeneralCommandLine =
        GeneralCommandLine(ParametersListUtil.parse(project.getService(IrisLspSettings::class.java).command))
            .withWorkDirectory(project.basePath)
            .withParentEnvironmentType(GeneralCommandLine.ParentEnvironmentType.CONSOLE)
}
