package examples;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

public final class Output {
  public static void main(String[] args) throws IOException {
    Spec spec = Spec.parse(Files.readAllBytes(Path.of("formats/dimacs-cnf/dimacs-cnf.iris")));
    Data data = spec.read(Path.of("formats/dimacs-cnf/cases/three-variables/input.txt"));

    System.out.print(data.toDzn());
    System.out.print(data.toJson());
  }
}
