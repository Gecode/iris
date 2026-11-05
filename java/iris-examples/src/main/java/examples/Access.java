package examples;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.Value;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

public final class Access {
  public static void main(String[] args) throws IOException {
    Spec spec = Spec.parse(Files.readAllBytes(Path.of("formats/dimacs-cnf/dimacs-cnf.iris")));
    Data data = spec.read(Path.of("formats/dimacs-cnf/cases/three-variables/input.txt"));

    for (Data.Entry entry : data.entries()) {
      Value value = entry.value();
      System.out.println(entry.name() + " " + value.type().text() + " " + value.depth());
    }
    System.out.println((data.find("clause") != null) + " " + (data.find("model") != null));

    Value clauses = data.get("clause");
    System.out.println(clauses.size() + " " + clauses.get(1).size());
    System.out.println(clauses.at(1, 0).asLong());
    for (int i = 0; i < clauses.size(); i++) {
      System.out.println("clause " + i + ": " + Arrays.toString(clauses.get(i).ints()));
    }

    System.out.println(describe(data.get("vars")));
    System.out.println(describe(clauses));

    try {
      System.out.println(clauses.asLong());
    } catch (IllegalStateException e) {
      System.out.println(e.getMessage());
    }
  }

  private static String describe(Value value) {
    return switch (value) {
      case Value.IntValue(long integer) -> "the integer " + integer;
      case Value.FloatValue(double number) -> "the float " + number;
      case Value.StringValue(String text) -> "the string " + text;
      case Value.ArrayValue array -> "an array of " + array.size();
    };
  }
}
