package examples;

import dev.gecode.iris.Spec;
import dev.gecode.iris.driver.Driver;
import java.io.IOException;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

/**
 * A conformance driver that answers each request with the spec of a format's builder, in place of
 * the format's {@code .iris} file. Its arguments are the builder's language, {@code java} or {@code
 * kotlin}, the format's directory name, and then the driver protocol's arguments.
 */
final class FormatBuilderDriver {
  /** The languages with a builder for every format. */
  static final List<String> LANGUAGES = List.of("java", "kotlin");

  /** The formats of the format library, by directory name. */
  static final List<String> FORMATS =
      List.of("dimacs-cnf", "jobshop", "nonogram", "sudoku", "tsplib", "tsptw");

  private FormatBuilderDriver() {}

  public static void main(String[] args) {
    if (args.length < 2) {
      System.err.println("usage: FormatBuilderDriver java|kotlin FORMAT ARGUMENTS...");
      System.exit(2);
    }
    String format = args[1];
    Spec spec = spec(args[0], format);
    String[] request = Arrays.copyOfRange(args, 2, args.length);
    int status =
        Driver.run(
            request,
            System.out,
            System.err,
            path -> {
              // A case's spec.iris links to its format's spec, and a case of another format
              // would read with the wrong spec.
              Path file = Path.of(path).toRealPath().getFileName();
              if (file == null || !file.toString().equals(format + ".iris")) {
                throw new IOException("the " + format + " builder does not answer for " + path);
              }
              return spec;
            });
    System.exit(status);
  }

  /** {@return the spec of the builder of {@code format} in {@code language}} */
  static Spec spec(String language, String format) {
    return switch (language) {
      case "java" ->
          switch (format) {
            case "dimacs-cnf" -> BuildDimacsCnf.spec;
            case "jobshop" -> BuildJobshop.spec;
            case "nonogram" -> BuildNonogram.spec;
            case "sudoku" -> BuildSudoku.spec;
            case "tsplib" -> BuildTsplib.spec;
            case "tsptw" -> BuildTsptw.spec;
            default -> throw new IllegalArgumentException("no Java builder for " + format);
          };
      case "kotlin" ->
          switch (format) {
            case "dimacs-cnf" -> BuildDimacsCnfKt.getDimacsCnf().getSpec();
            case "jobshop" -> BuildJobshopKt.getJobshop().getSpec();
            case "nonogram" -> BuildNonogramKt.getNonogram().getSpec();
            case "sudoku" -> BuildSudokuKt.getSudoku().getSpec();
            case "tsplib" -> BuildTsplibKt.getTsplib().getSpec();
            case "tsptw" -> BuildTsptwKt.getTsptw().getSpec();
            default -> throw new IllegalArgumentException("no Kotlin builder for " + format);
          };
      default -> throw new IllegalArgumentException("no builders in " + language);
    };
  }
}
