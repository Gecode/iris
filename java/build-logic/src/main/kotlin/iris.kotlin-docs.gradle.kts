plugins { id("org.jetbrains.dokka") }

dokka {
  moduleName = "iris-kotlin"
  dokkaPublications.html { failOnWarning = true }
  dokkaSourceSets.configureEach { jdkVersion = 27 }
}

tasks.named("check") { dependsOn("dokkaGenerate") }
