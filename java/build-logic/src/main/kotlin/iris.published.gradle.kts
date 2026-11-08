import com.vanniktech.maven.publish.MavenPublishBaseExtension

plugins {
  `java-library`
  id("com.vanniktech.maven.publish")
}

// The plugin picks the publication from the applied plugins after evaluation: javadoc and sources
// jars for a Java library, a Dokka javadoc jar and sources for a Kotlin one. Each module sets its
// description. publishToMavenCentral uploads a deployment to the Central Portal for a manual
// release, publishAndReleaseToMavenCentral releases it too, and both read the Portal user token
// from the mavenCentralUsername and mavenCentralPassword properties.
configure<MavenPublishBaseExtension> {
  publishToMavenCentral()
  pom {
    name = project.name
    description = provider { project.description }
    url = "https://iris.gecode.dev"
    inceptionYear = "2026"
    licenses {
      license {
        name = "MIT License"
        url = "https://opensource.org/license/mit"
        distribution = "repo"
      }
    }
    developers {
      developer {
        id = "zayenz"
        name = "Mikael Zayenz Lagerkvist"
        url = "https://github.com/zayenz"
      }
    }
    scm {
      url = "https://github.com/Gecode/iris"
      connection = "scm:git:https://github.com/Gecode/iris.git"
      developerConnection = "scm:git:ssh://git@github.com/Gecode/iris.git"
    }
  }
}

// Signing needs the ASCII-armored key in signingInMemoryKey, and its passphrase in
// signingInMemoryKeyPassword, usually from ORG_GRADLE_PROJECT_ variables. Without a key,
// publishToMavenLocal writes unsigned files and an upload to Maven Central fails before it starts.
if (providers.gradleProperty("signingInMemoryKey").isPresent) {
  configure<MavenPublishBaseExtension> { signAllPublications() }
} else {
  tasks.withType<PublishToMavenRepository>().configureEach {
    if (name.endsWith("ToMavenCentralRepository")) {
      doFirst { throw GradleException("Maven Central needs signatures: set signingInMemoryKey") }
    }
  }
}
