package dev.gecode.iris.internal.access;

import dev.gecode.iris.IrisError;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;
import java.lang.invoke.MethodHandles;
import org.jspecify.annotations.Nullable;

/**
 * The parts of {@link Spec} and {@link IrisException} that the module's other packages use and the
 * public API does not show: the statement an error belongs to, and the call sites of a reader spec
 * from a spec builder. {@link Spec} grants them when it is initialized.
 */
public final class Access {
  private Access() {}

  /** What {@link Spec} grants. */
  public interface Grants {
    /**
     * Returns an exception reporting {@code error}, which belongs to the statement on {@code
     * ownerLine}.
     */
    IrisException failure(IrisError error, int ownerLine);

    /** {@return the reader-spec line of the statement that {@code failure} belongs to, or 0} */
    int ownerLine(IrisException failure);

    /**
     * Parses {@code text} as {@link Spec#parse(String, String)} does and keeps {@code sites}, so
     * that every error of the reader spec, a parse error included, names the call site of its
     * statement.
     */
    Spec parse(String text, String origin, CallSites sites);

    /** {@return the call sites of {@code spec}} */
    CallSites callSites(Spec spec);
  }

  private static volatile @Nullable Grants grants;

  /**
   * Records what {@link Spec} grants.
   *
   * @throws IllegalStateException if something was granted already
   */
  public static void grant(Grants granted) {
    synchronized (Access.class) {
      if (grants != null) {
        throw new IllegalStateException("access is granted once");
      }
      grants = granted;
    }
  }

  /**
   * Returns an exception reporting {@code error}, which belongs to the statement on reader-spec
   * line {@code ownerLine}, or to none if it is 0.
   */
  public static IrisException failure(IrisError error, int ownerLine) {
    return grants().failure(error, ownerLine);
  }

  /** {@return the reader-spec line of the statement that {@code failure} belongs to, or 0} */
  public static int ownerLine(IrisException failure) {
    return grants().ownerLine(failure);
  }

  /** Parses the text of a spec builder, as {@link Grants#parse} says. */
  public static Spec parse(String text, String origin, CallSites sites) {
    return grants().parse(text, origin, sites);
  }

  /** {@return the call sites of {@code spec}, which are {@link CallSites#NONE} for parsed text} */
  public static CallSites callSites(Spec spec) {
    return grants().callSites(spec);
  }

  private static Grants grants() {
    Grants granted = grants;
    if (granted != null) {
      return granted;
    }
    try {
      MethodHandles.lookup().ensureInitialized(Spec.class);
    } catch (IllegalAccessException e) {
      throw new IllegalStateException(e);
    }
    granted = grants;
    if (granted == null) {
      throw new IllegalStateException("Spec grants no access");
    }
    return granted;
  }
}
