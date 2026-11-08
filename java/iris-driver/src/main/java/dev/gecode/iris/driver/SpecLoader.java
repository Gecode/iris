package dev.gecode.iris.driver;

import dev.gecode.iris.Spec;
import java.io.IOException;

/** Gives the spec that a {@link Driver} request reads with. */
@FunctionalInterface
public interface SpecLoader {
  /**
   * Returns the spec for the request's spec path.
   *
   * @param path the spec path of the request
   * @return the spec
   * @throws IOException if the spec cannot be read
   */
  Spec load(String path) throws IOException;
}
