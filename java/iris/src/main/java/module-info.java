import org.jspecify.annotations.NullMarked;

/** Reads text instance files described by Iris reader specs. */
@NullMarked
module dev.gecode.iris {
  requires static transitive org.jspecify;

  exports dev.gecode.iris;
}
