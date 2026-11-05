package dev.gecode.iris.internal.read;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisError;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Location;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.SequencedMap;
import org.jspecify.annotations.Nullable;

/**
 * The state of a read that its parts share: the input, the values defined so far, and where in the
 * reader spec it is, which every error reports.
 */
final class Execution {
  private final Input input;
  private final @Nullable String specOrigin;
  private final Budget budget;

  /** The values: the outermost frame, and one per active repeat iteration or instance. */
  private final List<SequencedMap<String, Stored>> frames = new ArrayList<>();

  private final List<Context> contexts = new ArrayList<>();
  private int specLine = 0;

  /**
   * A statement being executed, for errors.
   *
   * @param text what is executed, such as a name
   * @param iteration the iteration of a repeat or instances statement, from 1, or 0
   * @param iterations the number of iterations, or -1 if it is unknown
   */
  record Context(String text, int iteration, int iterations) {
    /** Returns a context without iterations. */
    static Context of(String text) {
      return new Context(text, 0, -1);
    }
  }

  Execution(Input input, @Nullable String specOrigin) {
    this.input = input;
    this.specOrigin = specOrigin;
    this.budget = new Budget(input.bytes().length);
    frames.add(new LinkedHashMap<>());
  }

  Input input() {
    return input;
  }

  /** Returns {@code text} in single quotes, with newlines and tabs escaped. */
  static String quote(String text) {
    return "'" + text.replace("\n", "\\n").replace("\t", "\\t") + "'";
  }

  /*
   * Where the read is
   */

  void setSpecLine(int line) {
    specLine = line;
  }

  void enter(Context context) {
    contexts.add(context);
  }

  void leave() {
    contexts.removeLast();
  }

  /** Returns the text of the innermost context. */
  String contextText() {
    return contexts.getLast().text();
  }

  private @Nullable String contextPath() {
    if (contexts.isEmpty()) {
      return null;
    }
    StringBuilder path = new StringBuilder();
    for (Context context : contexts) {
      path.append(path.isEmpty() ? "" : " > ").append(context.text());
      if (context.iteration() > 0) {
        path.append(" [").append(context.iteration());
        if (context.iterations() >= 0) {
          path.append(" of ").append(context.iterations());
        }
        path.append(']');
      }
    }
    return path.toString();
  }

  private Location inputLocation(Place at) {
    return new Location(input.origin(), at.line(), at.column());
  }

  private Location specLocation() {
    return new Location(specOrigin, specLine, null);
  }

  /*
   * Errors
   */

  /** Returns the error {@code code} at the current position (§11.2 profile R). */
  IrisException failure(ErrorCode code, String message) {
    return located(code, message, input.here());
  }

  /** Returns the error {@code code} at {@code at} (§11.2 profile R). */
  IrisException located(ErrorCode code, String message, Place at) {
    return new IrisException(
        new IrisError.ReadError(
            code, message, inputLocation(at), specLine > 0 ? specLocation() : null, contextPath()));
  }

  /** Returns the error of a shape error of the value of {@code name}. */
  IrisException failure(ShapeError error, String name) {
    return failure(error.code(), "'" + name + "': " + error.phrase());
  }

  /** Returns the failed check {@code message} at {@code at} with the witness {@code path}. */
  IrisException checkFailure(String message, Place at, List<Integer> path) {
    return new IrisException(
        new IrisError.CheckError(
            ErrorCode.CHECK_FAILED,
            message,
            inputLocation(at),
            specLocation(),
            contextPath(),
            path));
  }

  /** Returns the error {@code code} at the current position, with no location in the spec. */
  IrisException inputFailure(ErrorCode code, String message) {
    return new IrisException(
        new IrisError.ReadError(code, message, inputLocation(input.here()), null, null));
  }

  /** Returns the error {@code code} about the reader spec as a whole. */
  IrisException specFailure(ErrorCode code, String message) {
    return new IrisException(
        new IrisError.ReadError(code, message, null, new Location(specOrigin, null, null), null));
  }

  /*
   * Budgets
   */

  /** Accounts for {@code units} of work that read no input (§10.2). */
  void chargeIdle(long units) {
    if (!budget.chargeIdle(units)) {
      throw failure(ErrorCode.BUDGET, "too many repetitions or empty rows without reading input");
    }
  }

  /** Accounts for visiting {@code units} values in expressions (§10.3). */
  void chargeEffort(long units) {
    if (!budget.chargeEffort(units)) {
      throw failure(ErrorCode.BUDGET, "the expressions do too much work for the size of the input");
    }
  }

  /*
   * Values
   */

  /** Defines {@code name} as {@code value} in the innermost frame. */
  void define(String name, Stored value) {
    frames.getLast().put(name, value);
  }

  /** Returns the value of {@code name}, innermost frame first, or null if it is not defined. */
  @Nullable Stored lookup(String name) {
    for (int k = frames.size() - 1; k >= 0; k--) {
      Stored value = frames.get(k).get(name);
      if (value != null) {
        return value;
      }
    }
    return null;
  }

  /** Makes {@code frame} the innermost frame. */
  void pushFrame(SequencedMap<String, Stored> frame) {
    frames.add(frame);
  }

  /** Removes the innermost frame and returns it. */
  SequencedMap<String, Stored> popFrame() {
    return frames.removeLast();
  }

  /** Returns the index of the innermost frame. */
  int innermostFrame() {
    return frames.size() - 1;
  }

  /** Returns the frame at {@code index}. */
  SequencedMap<String, Stored> frame(int index) {
    return frames.get(index);
  }
}
