plugins {
  `java-library`
  `maven-publish`
}

// Enough for publishToMavenLocal. Releasing to Maven Central adds its metadata and signatures.
publishing { publications { create<MavenPublication>("maven") { from(components["java"]) } } }
