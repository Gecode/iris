/** Kotlin extensions for the data that Iris reads, and a Kotlin builder of reader specs. */
module dev.gecode.iris.kotlin {
  requires transitive dev.gecode.iris;
  requires transitive kotlin.stdlib;

  exports dev.gecode.iris.kotlin;
  exports dev.gecode.iris.kotlin.build;
}
