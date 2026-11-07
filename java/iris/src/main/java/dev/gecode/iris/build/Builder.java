package dev.gecode.iris.build;

import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;
import dev.gecode.iris.internal.access.Access;

/**
 * The statements of a reader spec, from {@link Iris#builder}. {@link #source} prints them, and
 * {@link #build} parses the text into an ordinary {@link Spec}.
 */
public final class Builder {
  /** The origin reported for the text a builder emits. */
  static final String ORIGIN = "builder.iris";

  private final State state;

  Builder(State state) {
    this.state = state;
  }

  /**
   * {@return the text of the reader spec, as {@code iris fmt} writes it with no line limit}
   *
   * @throws BuildException if a check of the whole spec fails, or an earlier statement failed one
   */
  public String source() {
    return printed().text();
  }

  /**
   * Prints the text and parses it.
   *
   * <p>The spec keeps the call site of each statement, so that an error from reading with it names
   * the line of the caller's code that wrote the statement the error belongs to, in {@link
   * IrisException#callSite}.
   *
   * @return the reader spec
   * @throws BuildException if a check fails, or the language rejects the text; then {@link
   *     BuildException#callSite} names the call of the statement that the error belongs to
   */
  public Spec build() {
    Printer.Printed printed = printed();
    try {
      return Access.parse(printed.text(), ORIGIN, printed.sites());
    } catch (IrisException failure) {
      throw BuildException.spec(failure, printed.text(), failure.callSite().orElse(null));
    }
  }

  private Printer.Printed printed() {
    if (state.issue != null) {
      throw state.issue;
    }
    Walk.check(state);
    return Printer.print(state);
  }
}
