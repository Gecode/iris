pluginManagement { includeBuild("build-logic") }

dependencyResolutionManagement { repositories { mavenCentral() } }

rootProject.name = "iris-java"

include(
    "iris",
    "iris-driver",
    "iris-examples",
    "iris-jmh",
    "iris-kotlin",
    "iris-kotlin-round-trip",
    "iris-round-trip",
)
