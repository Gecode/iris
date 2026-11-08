package dev.gecode.iris.kotlin

import java.io.File
import java.lang.module.ModuleFinder
import java.nio.file.Path
import kotlin.io.path.Path
import kotlin.io.path.name
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ModuleTest {
    private val moduleName = "dev.gecode.iris.kotlin"

    /** The module's jar, with the jars of the modules it requires from the test's class path. */
    private fun modulePath(): Array<Path> {
        val required =
            System.getProperty("java.class.path").split(File.pathSeparator).map(::Path).filter {
                it.name.startsWith("iris-") || it.name.startsWith("kotlin-stdlib-")
            }
        return (listOf(Path(System.getProperty("iris.kotlinJar"))) + required).toTypedArray()
    }

    @Test
    fun resolvesToTheLibraryAndTheStandardLibraryAlone() {
        val configuration =
            ModuleLayer.boot()
                .configuration()
                .resolve(ModuleFinder.of(*modulePath()), ModuleFinder.of(), setOf(moduleName))
        assertEquals(
            setOf(moduleName, "dev.gecode.iris", "kotlin.stdlib"),
            configuration.modules().map { it.name() }.toSet(),
        )
    }

    @Test
    fun exportsTheExtensionsAndTheBuilder() {
        val descriptor = ModuleFinder.of(*modulePath()).find(moduleName).get().descriptor()
        val packages = setOf(moduleName, "$moduleName.build")
        assertEquals(packages, descriptor.exports().map { it.source() }.toSet())
        assertTrue(descriptor.exports().none { it.isQualified })
        assertEquals(packages, descriptor.packages())
    }
}
