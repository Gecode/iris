import org.jetbrains.intellij.platform.gradle.tasks.PrepareSandboxTask
import org.jetbrains.intellij.platform.gradle.TestFrameworkType
import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType

plugins {
    kotlin("jvm") version "2.2.20"
    id("org.jetbrains.intellij.platform") version "2.19.0"
}

group = "org.gecode.iris"
version = "0.1.0-SNAPSHOT"

repositories {
    mavenCentral()
    intellijPlatform { defaultRepositories() }
}

dependencies {
    intellijPlatform {
        intellijIdea("2025.3.3")
        bundledPlugin("org.jetbrains.plugins.textmate")
        testFramework(TestFrameworkType.Platform)
        pluginVerifier()
    }
    testImplementation("junit:junit:4.13.2")
}

kotlin { jvmToolchain(21) }

intellijPlatform {
    sandboxContainer = layout.buildDirectory.dir("sandbox")
    buildSearchableOptions = false
    pluginConfiguration {
        ideaVersion {
            sinceBuild = "253"
            untilBuild = "262.*"
        }
    }
    pluginVerification {
        ides {
            // Representative ends of the past-year window; override for other products.
            create(IntelliJPlatformType.IntellijIdea, "2025.3.3")
            create(IntelliJPlatformType.IntellijIdea, "2026.2.4")
        }
    }
}

tasks.processResources {
    // Preserve the tuned brand artwork; only set its display dimensions.
    from("../../brand/favicon/favicon.svg") {
        into("icons")
        rename { "iris.svg" }
        filter { it.replace("<svg ", "<svg width=\"16\" height=\"16\" ") }
    }
    from("../../brand/favicon/favicon.svg") {
        into("META-INF")
        rename { "pluginIcon.svg" }
        filter { it.replace("<svg ", "<svg width=\"40\" height=\"40\" ") }
    }
}

// Keep one tracked grammar. Both the ZIP and test sandbox contain this directory.
val prepareTextMateBundle = tasks.register<Sync>("prepareTextMateBundle") {
    from("../textmate/iris.tmLanguage.json")
    from("src/main/textmate/package.json")
    into(layout.buildDirectory.dir("textmate/iris"))
}

tasks.withType<PrepareSandboxTask>().configureEach {
    from(prepareTextMateBundle) { into(pluginName.map { "$it/textmate/iris" }) }
}

tasks.test {
    providers.environmentVariable("IRIS_TEST_EXECUTABLE").orNull?.let { systemProperty("iris.executable", it) }
    systemProperty("iris.stub", layout.projectDirectory.file("src/test/resources/stub_server.py").asFile.absolutePath)
    systemProperty("idea.log.debug.categories", "#com.intellij.platform.lsp")
}
