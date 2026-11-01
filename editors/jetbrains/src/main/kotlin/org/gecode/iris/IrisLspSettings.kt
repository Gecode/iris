package org.gecode.iris

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.StoragePathMacros

@Service(Service.Level.PROJECT)
@State(name = "IrisLspSettings", storages = [Storage(StoragePathMacros.WORKSPACE_FILE)])
class IrisLspSettings : PersistentStateComponent<IrisLspSettings.Options> {
    class Options {
        var command: String = "iris lsp"
    }

    private var options = Options()

    var command: String
        get() = options.command
        set(value) { options.command = value }

    override fun getState(): Options = options
    override fun loadState(state: Options) { options = state }
}
