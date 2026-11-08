plugins {
    `java-library`
    id("iris.java-conventions")
    id("iris.published")
}

// The processor checks a spec and finds its entries with the library, including its internal
// syntax packages, so it has no module descriptor: it runs from the processor path, where the
// library is not a named module. The generated code needs only dev.gecode:iris.
dependencies { implementation(project(":iris")) }

tasks.jar { manifest { attributes("Automatic-Module-Name" to "dev.gecode.iris.processor") } }

// The internal packages are not part of the library's API; javadoc documents only this package.
tasks.javadoc { (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true) }

tasks.check { dependsOn(tasks.javadoc) }

tasks.test {
    systemProperty("iris.repository", rootDir.parentFile.path)
    inputs.dir(rootDir.parentFile.resolve("formats")).withPathSensitivity(PathSensitivity.RELATIVE)
}
