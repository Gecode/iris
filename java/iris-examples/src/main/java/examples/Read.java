package examples;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import java.util.Arrays;

public final class Read {
  public static void main(String[] args) {
    Spec spec = Spec.parse("n:int row[n]:int", "example.iris");
    Data data = spec.read("3 10 20 30", "input.txt");

    System.out.println(data.get("n").asLong());
    System.out.println(Arrays.toString(data.get("row").longs()));
  }
}
