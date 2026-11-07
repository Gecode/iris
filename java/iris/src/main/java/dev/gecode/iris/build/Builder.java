package dev.gecode.iris.build;

import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;

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
   * @return the reader spec
   * @throws BuildException if a check fails, or the language rejects the text; then {@link
   *     BuildException#callSite} names the call of the statement on the line the error points at
   */
  public Spec build() {
    Printer.Printed printed = printed();
    try {
      return Spec.parse(printed.text(), ORIGIN);
    } catch (IrisException failure) {
      Integer line = failure.line();
      throw BuildException.spec(
          failure, printed.text(), line == null ? null : printed.siteOfLine(line));
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
