package dev.gecode.iris.build;

import java.lang.StackWalker.StackFrame;
import java.net.URL;
import java.security.CodeSource;
import java.util.Objects;
import org.jspecify.annotations.Nullable;

/** Call sites: where in the host program each statement was called. */
final class Site {
  private Site() {}

  private static final StackWalker WALKER =
      StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE);

  private static final String PACKAGE = Site.class.getPackageName();

  private static final @Nullable URL LOCATION = location(Site.class);

  static final StackTraceElement UNKNOWN =
      new StackTraceElement("<unknown>", "<unknown>", null, -1);

  /** Whether a class is part of this package's implementation, as opposed to its callers. */
  private static final ClassValue<Boolean> INTERNAL =
      new ClassValue<>() {
        @Override
        protected Boolean computeValue(Class<?> type) {
          String name = type.getPackageName();
          if (name.startsWith("java.") || name.startsWith("jdk.")) {
            return true;
          }
          return (name.equals(PACKAGE) && Objects.equals(location(type), LOCATION))
              || tracksCaller(type);
        }
      };

  /** Whether {@code type} or a class enclosing it carries {@link TrackCaller}. */
  private static boolean tracksCaller(Class<?> type) {
    for (Class<?> c = type; c != null; c = c.getEnclosingClass()) {
      if (c.isAnnotationPresent(TrackCaller.class)) {
        return true;
      }
    }
    return false;
  }

  private static @Nullable URL location(Class<?> type) {
    CodeSource source = type.getProtectionDomain().getCodeSource();
    return source == null ? null : source.getLocation();
  }

  /**
   * The innermost frame outside this package, the platform and the classes marked {@link
   * TrackCaller}. A class in this package from another code source, such as a test, counts as a
   * caller.
   */
  static StackTraceElement caller() {
    return WALKER
        .walk(frames -> frames.filter(Site::isCaller).findFirst())
        .map(StackFrame::toStackTraceElement)
        .orElse(UNKNOWN);
  }

  private static boolean isCaller(StackFrame frame) {
    return !INTERNAL.get(frame.getDeclaringClass());
  }
}
