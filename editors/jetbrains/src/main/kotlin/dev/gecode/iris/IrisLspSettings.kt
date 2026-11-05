package dev.gecode.iris

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.StoragePathMacros

@Service(Service.Level.PROJECT)
@State(name = "IrisLspSettings", storages = [Storage(StoragePathMacros.WORKSPACE_FILE)])
class IrisLspSettings : PersistentStateComponent<IrisLspSettings.Options> {
    class Options {
        var executablePath: String = ""
    }

    private var options = Options()

    var executablePath: String
        get() = options.executablePath
        set(value) { options.executablePath = value }

    override fun getState(): Options = options
    override fun loadState(state: Options) { options = state }
}
