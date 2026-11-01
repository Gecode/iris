package org.gecode.iris

import com.intellij.openapi.extensions.PluginAware
import com.intellij.openapi.extensions.PluginDescriptor
import org.jetbrains.plugins.textmate.api.TextMateBundleProvider

class IrisTextMateBundleProvider : TextMateBundleProvider, PluginAware {
    private lateinit var plugin: PluginDescriptor

    override fun setPluginDescriptor(pluginDescriptor: PluginDescriptor) { plugin = pluginDescriptor }

    override fun getBundles(): List<TextMateBundleProvider.PluginBundle> =
        listOf(TextMateBundleProvider.PluginBundle("Iris", plugin.pluginPath.resolve("textmate/iris")))
}
