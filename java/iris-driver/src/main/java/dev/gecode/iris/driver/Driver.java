package dev.gecode.iris.driver;

import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.Data;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.Serial;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** Answers one request of the driver protocol in {@code conformance/README.md}. */
final class Driver {
  private static final int LANGUAGE_ERROR = 1;
  private static final int MISUSE = 2;
  private static final int HOST_FAILURE = 3;

  private Driver() {}

  /**
   * Runs the request {@code args}, writing a document to {@code out} and diagnostics to {@code
   * err}.
   *
   * @return the exit status of the protocol
   */
  static int run(String[] args, OutputStream out, PrintStream err) {
    try {
      Response response = respond(Request.of(args));
      out.write(response.document().getBytes(UTF_8));
      out.flush();
      return response.status();
    } catch (Misuse e) {
      err.println("iris-driver: " + e.getMessage());
      return MISUSE;
    } catch (IOException | RuntimeException | OutOfMemoryError e) {
      err.println("iris-driver: " + e);
      return HOST_FAILURE;
    }
  }

  /**
   * Refuses an argument that the JVM may have changed. It decodes arguments in the locale's
   * encoding before {@code main} runs, so only ASCII survives outside a UTF-8 locale. Changing the
   * locale for the JVM instead would break file names in the original encoding.
   *
   * <p>On Windows {@code java} reads its command line in the ANSI code page, whatever the console's
   * code page is. Characters outside that code page have become other characters, often {@code ?},
   * before this check sees them, so it only catches those inside the code page.
   */
  static void requireIntact(String argument, String encoding) {
    if (!UTF_8.name().equalsIgnoreCase(encoding) && !argument.chars().allMatch(c -> c < 0x80)) {
      throw new IllegalStateException(
          "non-ASCII --params needs a UTF-8 locale or JSON escapes; this one uses " + encoding);
    }
  }

  /** A document for standard output, with the exit status of the request. */
  private record Response(String document, int status) {}

  private static Response respond(Request request) throws IOException {
    byte[] source = readFile(request.spec());
    try {
      Spec spec = Spec.parse(source, request.spec());
      return switch (request.operation()) {
        case PARSE -> new Response(Json.PARSED, 0);
        case READ -> new Response(read(spec, request).toJson(), 0);
        case CATALOG -> {
          byte[] input = readFile(request.input());
          Data data = spec.catalog(input, request.input(), request.params());
          yield new Response(request.format() == Format.DZN ? data.toDzn() : data.toJson(), 0);
        }
        case DZN -> new Response(read(spec, request).toDzn(), 0);
      };
    } catch (IrisException e) {
      return new Response(e.error().toJson(), LANGUAGE_ERROR);
    }
  }

  private static byte[] readFile(String path) throws IOException {
    return Files.readAllBytes(Path.of(path));
  }

  private static Data read(Spec spec, Request request) throws IOException {
    byte[] input = readFile(request.input());
    return spec.read(input, request.input(), request.params());
  }

  /** The operations of the protocol. */
  private enum Operation {
    PARSE,
    READ,
    CATALOG,
    DZN
  }

  /** The documents that a catalog can answer with. */
  private enum Format {
    JSON,
    DZN
  }

  /** Thrown for arguments that are not a request of the protocol. */
  private static final class Misuse extends RuntimeException {
    @Serial private static final long serialVersionUID = 1L;

    Misuse(String message) {
      super(message);
    }
  }

  /**
   * A valid request.
   *
   * @param operation the operation
   * @param spec the path of the reader spec
   * @param input the path of the input, which is empty for a parse
   * @param params the parameters
   * @param format the document of a catalog
   */
  private record Request(
      Operation operation, String spec, String input, Params params, Format format) {
    private static final String USAGE =
        "usage: iris-driver parse|read|catalog|dzn SPEC [INPUT] "
            + "[--params JSON] [--version 0.1] [--format json|dzn]";

    static Request of(String[] args) {
      if (args.length < 2) {
        throw new Misuse(USAGE);
      }
      Operation operation = operation(args[0]);
      int positionals = operation == Operation.PARSE ? 2 : 3;
      if (args.length < positionals) {
        throw new Misuse("missing input path");
      }
      for (String path : List.of(args).subList(1, positionals)) {
        if (path.startsWith("--")) {
          throw new Misuse("missing path before " + path);
        }
      }
      List<String> options = List.of(args).subList(positionals, args.length);
      checkOptions(operation, options);
      String input = operation == Operation.PARSE ? "" : args[2];
      return new Request(operation, args[1], input, params(options), format(options));
    }

    private static Format format(List<String> options) {
      int option = options.indexOf("--format");
      return option >= 0 && options.get(option + 1).equals("dzn") ? Format.DZN : Format.JSON;
    }

    private static Params params(List<String> options) {
      int option = options.indexOf("--params");
      if (option < 0) {
        return Params.of();
      }
      String json = options.get(option + 1);
      requireIntact(json, System.getProperty("sun.jnu.encoding"));
      try {
        return Params.of(Json.params(json));
      } catch (IllegalArgumentException e) {
        throw new Misuse(e.getMessage());
      }
    }

    private static Operation operation(String command) {
      return switch (command) {
        case "parse" -> Operation.PARSE;
        case "read" -> Operation.READ;
        case "catalog" -> Operation.CATALOG;
        case "dzn" -> Operation.DZN;
        default -> throw new Misuse("unknown command: " + command);
      };
    }

    private static void checkOptions(Operation operation, List<String> options) {
      Set<String> seen = new HashSet<>();
      for (int k = 0; k < options.size(); k += 2) {
        String option = options.get(k);
        if (!option.equals("--version")
            && !option.equals("--params")
            && !option.equals("--format")) {
          throw new Misuse("unknown option or extra argument: " + option);
        }
        if (!seen.add(option)) {
          throw new Misuse("repeated option: " + option);
        }
        if (k + 1 == options.size()) {
          throw new Misuse("missing value for option: " + option);
        }
        checkOption(operation, option, options.get(k + 1));
      }
    }

    private static void checkOption(Operation operation, String option, String value) {
      switch (option) {
        case "--version" -> {
          if (!value.equals("0.1")) {
            throw new Misuse("unsupported language version: " + value);
          }
        }
        case "--format" -> {
          if (operation != Operation.CATALOG || !(value.equals("json") || value.equals("dzn"))) {
            throw new Misuse("--format json|dzn is only valid for catalog");
          }
        }
        default -> {
          if (operation == Operation.PARSE && !isEmptyObject(value)) {
            throw new Misuse("parse requires empty params");
          }
        }
      }
    }

    private static boolean isEmptyObject(String json) {
      return json.replaceAll("[ \\t\\r\\n]", "").equals("{}");
    }
  }
}
