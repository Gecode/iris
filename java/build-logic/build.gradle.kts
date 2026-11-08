plugins { `kotlin-dsl` }

dependencies {
  implementation(libs.spotless.plugin)
  implementation(libs.errorprone.plugin)
  implementation(libs.nullaway.plugin)
  implementation(libs.kotlin.plugin)
  implementation(libs.detekt.plugin)
  implementation(libs.dokka.plugin)
  implementation(libs.maven.publish.plugin)
}
