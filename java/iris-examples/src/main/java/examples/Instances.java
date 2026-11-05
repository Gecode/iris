package examples;

import dev.gecode.iris.Data;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

public final class Instances {
  public static void main(String[] args) throws IOException {
    Spec spec = Spec.parse(Files.readAllBytes(Path.of("formats/jobshop/jobshop.iris")));
    Path input = Path.of("formats/jobshop/cases/or-library/input.txt");

    Data catalog = spec.catalog(input);
    System.out.print(catalog.toDzn());

    Data data = spec.read(input, Params.of("instance", "handmade-or"));
    System.out.println(data.get("n_jobs").asLong() + " " + data.get("n_machines").asLong());
  }
}
