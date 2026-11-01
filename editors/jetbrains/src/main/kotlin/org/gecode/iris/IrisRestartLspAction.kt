package org.gecode.iris

import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.platform.lsp.api.LspServerManager

class IrisRestartLspAction : AnAction() {
    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(event: AnActionEvent) { event.presentation.isEnabledAndVisible = event.project != null }

    override fun actionPerformed(event: AnActionEvent) {
        val project = event.project ?: return
        project.getService(IrisLspNotifications::class.java).clear()
        LspServerManager.getInstance(project).stopAndRestartIfNeeded(IrisLspSupportProvider::class.java)
    }
}
