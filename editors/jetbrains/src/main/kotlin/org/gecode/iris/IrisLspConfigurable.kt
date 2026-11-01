package org.gecode.iris

import com.intellij.openapi.options.Configurable
import com.intellij.openapi.project.Project
import com.intellij.platform.lsp.api.LspServerManager
import com.intellij.ui.components.JBLabel
import com.intellij.ui.components.JBTextField
import com.intellij.util.ui.FormBuilder
import javax.swing.JComponent

class IrisLspConfigurable(private val project: Project) : Configurable {
    private val settings = project.getService(IrisLspSettings::class.java)
    private var executableField: JBTextField? = null

    override fun getDisplayName(): String = "Iris"

    override fun createComponent(): JComponent {
        val field = JBTextField(settings.executablePath)
        executableField = field
        return FormBuilder.createFormBuilder()
            .addLabeledComponent("Executable path:", field)
            .addComponent(JBLabel("Leave empty to find iris on PATH. The plugin adds lsp; enter a path without quotes or arguments."))
            .addComponent(JBLabel("Applying changes restarts the server for open reader specs."))
            .panel
    }

    override fun isModified(): Boolean = executableField?.text?.let { it.trim() != settings.executablePath } ?: false

    override fun apply() {
        val executable = executableField?.text?.trim() ?: return
        if (executable == settings.executablePath) return
        settings.executablePath = executable
        project.getService(IrisLspNotifications::class.java).clear()
        LspServerManager.getInstance(project).stopAndRestartIfNeeded(IrisLspSupportProvider::class.java)
    }

    override fun reset() { executableField?.text = settings.executablePath }
    override fun disposeUIResources() { executableField = null }
}
