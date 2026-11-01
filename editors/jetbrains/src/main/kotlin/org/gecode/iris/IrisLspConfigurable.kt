package org.gecode.iris

import com.intellij.openapi.options.ConfigurationException
import com.intellij.openapi.options.Configurable
import com.intellij.openapi.project.Project
import com.intellij.ui.components.JBLabel
import com.intellij.ui.components.JBTextField
import com.intellij.util.execution.ParametersListUtil
import com.intellij.util.ui.FormBuilder
import javax.swing.JComponent

class IrisLspConfigurable(project: Project) : Configurable {
    private val settings = project.getService(IrisLspSettings::class.java)
    private var commandField: JBTextField? = null

    override fun getDisplayName(): String = "Iris"

    override fun createComponent(): JComponent {
        val field = JBTextField(settings.command)
        commandField = field
        return FormBuilder.createFormBuilder()
            .addLabeledComponent("Server command:", field)
            .addComponent(JBLabel("Reopen the project after changing the command. Arguments use quotes; no shell is started."))
            .panel
    }

    override fun isModified(): Boolean = commandField?.text?.let { it != settings.command } ?: false

    override fun apply() {
        val command = commandField?.text?.trim() ?: return
        if (ParametersListUtil.parse(command).firstOrNull().isNullOrBlank()) {
            throw ConfigurationException("Enter the Iris server command.")
        }
        settings.command = command
    }

    override fun reset() { commandField?.text = settings.command }
    override fun disposeUIResources() { commandField = null }
}
