package dev.gecode.iris.internal.syntax;

import java.util.concurrent.ExecutionException;
import java.util.concurrent.FutureTask;
import java.util.function.Supplier;
import org.jspecify.annotations.Nullable;

/**
 * Runs recursive work on a stack of known size.
 *
 * <p>The legal nesting of §5.12 needs up to about 900 KiB of JVM stack, which is most of the
 * default stack of a thread, so the stack of the caller cannot be relied on. The size here leaves a
 * margin of four. Trees need no more: their walks recurse only as deep as the nesting of §5.12, as
 * {@link Ast.Expr} explains. A JVM may still ignore the requested size, which makes an overflow a
 * failure of the host rather than an error of the reader spec.
 */
public final class LargeStack {
  private static final long SIZE = 4L * 1024 * 1024;

  private LargeStack() {}

  /**
   * Returns the result of {@code work}, which runs to completion even if the caller is interrupted.
   * The interrupt stays pending for the caller.
   */
  public static <T> T call(Supplier<T> work) {
    var task = new FutureTask<>(work::get);
    new Thread(null, task, "iris-large-stack", SIZE).start();
    boolean interrupted = false;
    try {
      while (true) {
        try {
          return task.get();
        } catch (InterruptedException e) {
          interrupted = true;
        } catch (ExecutionException e) {
          throw rethrown(e.getCause());
        }
      }
    } finally {
      if (interrupted) {
        Thread.currentThread().interrupt();
      }
    }
  }

  /** Returns what to throw for {@code failure}, or throws it if it is an unchecked exception. */
  private static IllegalStateException rethrown(@Nullable Throwable failure) {
    if (failure instanceof RuntimeException exception) {
      throw exception;
    }
    if (failure instanceof StackOverflowError) {
      return new IllegalStateException("the JVM stack is too small for this reader spec", failure);
    }
    if (failure instanceof Error error) {
      throw error;
    }
    return new IllegalStateException(failure);
  }
}
