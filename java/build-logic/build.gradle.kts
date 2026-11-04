plugins { `kotlin-dsl` }

dependencies {
  implementation(libs.spotless.plugin)
  implementation(libs.errorprone.plugin)
  implementation(libs.nullaway.plugin)
}
