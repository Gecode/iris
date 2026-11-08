package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.Gecode;
import dev.gecode.iris.build.IntField;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

/** A DIMACS CNF reader: the number of variables and one row of literals per clause. */
public final class BuildDimacsCnf {
  public static final IntField<Long> vars = int0("vars");
  public static final IntField<Long> clauses = int0("clauses");
  public static final IntField<List<long[]>> clause = int2("clause");

  public static final Spec spec =
      spec(
          "dimacs-cnf",
          () -> {
            comment("c");
            stop("%"); // SATLIB files end with "%" and "0"
            line(
                "p cnf ",
                capture(vars, range(0).to(Gecode.intMax)),
                " ",
                capture(clauses, atLeast(0)));
            // The terminating 0 is not stored.
            repeat(clauses, () -> list(clause, range(vars.neg()).to(vars).until(0)));
          });

  private BuildDimacsCnf() {}

  public static void main(String[] args) {
    Data data = spec.read(Path.of("formats/dimacs-cnf/cases/three-variables/input.txt"));
    System.out.println(data.get(vars) + " variables");
    for (long[] literals : data.get(clause)) {
      System.out.println(Arrays.toString(literals));
    }
  }
}
