plugins {
    `java-library`
    id("iris.java-conventions")
    id("iris.published")
}

description = "Writes typed handles and a record for an Iris reader spec while javac compiles"

// The processor checks a spec and finds its entries with the library, including its internal
// syntax packages, so it has no module descriptor: it runs from the processor path, where the
// library is not a named module. The generated code needs only dev.gecode:iris.
dependencies {
    implementation(project(":iris"))
    // The tests hold the generated classes to google-java-format's layout.
    testImplementation(libs.google.java.format)
}

tasks.jar { manifest { attributes("Automatic-Module-Name" to "dev.gecode.iris.processor") } }

// The internal packages are not part of the library's API; javadoc documents only this package.
tasks.javadoc { (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true) }

tasks.check { dependsOn(tasks.javadoc) }

tasks.test {
    systemProperty("iris.repository", rootDir.parentFile.path)
    // google-java-format parses with javac's internal API.
    jvmArgs(
        listOf("api", "code", "file", "parser", "tree", "util").map {
            "--add-exports=jdk.compiler/com.sun.tools.javac.$it=ALL-UNNAMED"
        }
    )
    inputs.dir(rootDir.parentFile.resolve("formats")).withPathSensitivity(PathSensitivity.RELATIVE)
}
