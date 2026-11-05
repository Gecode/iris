package dev.gecode.iris

import com.intellij.lang.Language
import com.intellij.openapi.fileTypes.LanguageFileType
import com.intellij.openapi.util.IconLoader
import javax.swing.Icon

object IrisLanguage : Language("Iris")

object IrisFileType : LanguageFileType(IrisLanguage) {
    override fun getName(): String = "Iris"
    override fun getDescription(): String = "Iris reader spec"
    override fun getDefaultExtension(): String = "iris"
    override fun getIcon(): Icon = IconLoader.getIcon("/icons/iris.svg", IrisFileType::class.java)
}
