package dev.gecode.iris.roundtrip;

import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.build.Builder;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

/**
 * Runs the builder programs of the Java round trip. Each program is a class {@code
 * rt.NAME.SpecBuilder} with a method {@code specBuilder()}; the runner writes the spec that it
 * builds to {@code OUT/NAME.iris}, or why it built none to {@code OUT/NAME.error}.
 */
public final class Main {
  private Main() {}

  /**
   * Runs every program under {@code args[0]} and writes its result to {@code args[1]}.
   *
   * @param args the directory of the programs, then the output directory
   * @throws IOException if a directory cannot be read or a result written
   */
  public static void main(String[] args) throws IOException {
    Path programs = Path.of(args[0]).resolve("rt");
    Path out = Path.of(args[1]);
    Files.createDirectories(out);
    List<String> names;
    try (Stream<Path> entries = Files.list(programs)) {
      names = entries.map(entry -> entry.getFileName().toString()).sorted().toList();
    }
    for (String name : names) {
      try {
        Files.writeString(out.resolve(name + ".iris"), source(name), UTF_8);
      } catch (ReflectiveOperationException | RuntimeException | LinkageError failure) {
        Throwable cause =
            failure instanceof InvocationTargetException invocation
                ? invocation.getCause()
                : failure;
        Files.writeString(out.resolve(name + ".error"), String.valueOf(cause), UTF_8);
      }
    }
  }

  private static String source(String name) throws ReflectiveOperationException {
    Method method = Class.forName("rt." + name + ".SpecBuilder").getDeclaredMethod("specBuilder");
    method.setAccessible(true);
    return ((Builder) method.invoke(null)).source();
  }
}
