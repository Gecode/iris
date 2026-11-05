package examples;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.IrisError;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

public final class Errors {
  public static void main(String[] args) throws IOException {
    String specPath = "formats/dimacs-cnf/dimacs-cnf.iris";
    String inputPath = "formats/dimacs-cnf/cases/literal-out-of-range/input.txt";
    Spec spec = Spec.parse(Files.readAllBytes(Path.of(specPath)), specPath);

    try {
      spec.read(Files.readAllBytes(Path.of(inputPath)), inputPath);
    } catch (IrisException e) {
      System.out.println(e.code().code());
      System.out.println(e.code() == ErrorCode.EXPECTED_VALUE);
      System.out.println(e.error().message());
      System.out.println(e.line() + " " + e.column());
      System.out.println(e.error().context());
      System.out.println(stage(e.error()));
      System.out.println(e.getMessage());
    }
  }

  private static String stage(IrisError error) {
    return switch (error) {
      case IrisError.SpecError e -> "the reader spec is wrong";
      case IrisError.ReadError e -> "the input does not match";
      case IrisError.CheckError e -> "a check failed at " + e.recordPath();
    };
  }
}
