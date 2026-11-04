package dev.gecode.iris.driver;

import static java.nio.charset.StandardCharsets.UTF_8;

import dev.gecode.iris.IrisException;
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
    } catch (IOException | RuntimeException e) {
      err.println("iris-driver: " + e);
      return HOST_FAILURE;
    }
  }

  /** A document for standard output, with the exit status of the request. */
  private record Response(String document, int status) {}

  private static Response respond(Request request) throws IOException {
    byte[] source = Files.readAllBytes(Path.of(request.spec()));
    try {
      Spec.parse(source, request.spec());
    } catch (IrisException e) {
      return new Response(Json.error(e.error()), LANGUAGE_ERROR);
    }
    if (request.operation() != Operation.PARSE) {
      throw new UnsupportedOperationException(
          "the operation " + request.operation() + " is not implemented");
    }
    return new Response(Json.PARSED, 0);
  }

  /** The operations of the protocol. */
  private enum Operation {
    PARSE,
    READ,
    CATALOG,
    DZN
  }

  /** Thrown for arguments that are not a request of the protocol. */
  private static final class Misuse extends RuntimeException {
    @Serial private static final long serialVersionUID = 1L;

    Misuse(String message) {
      super(message);
    }
  }

  /** A valid request, with what its execution needs so far. */
  private record Request(Operation operation, String spec) {
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
      checkOptions(operation, List.of(args).subList(positionals, args.length));
      return new Request(operation, args[1]);
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
