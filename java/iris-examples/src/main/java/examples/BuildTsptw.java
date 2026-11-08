package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.Gecode;
import dev.gecode.iris.build.IntField;
import dev.gecode.iris.build.RecordName;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

/**
 * A reader for the travelling salesman problem with time windows: a distance matrix and a time
 * window for each node, with the depot as node 0.
 */
public final class BuildTsptw {
  public static final IntField<Long> n = int0("n");
  public static final IntField<List<long[]>> dist = int2("dist");
  public static final IntField<long[]> ready = int1("ready");
  public static final IntField<long[]> due = int1("due");

  private static final RecordName node = recordName("node", "nodes");

  public static final Spec spec =
      spec(
          "tsptw",
          () -> {
            comment("#");
            read(n, range(2).to(1000));
            read(dist, range(0).to(Gecode.intMax.div(2).div(n)).dim(n).dim(n));

            var time = defineType("time", range(0).to(Gecode.intMax.div(2)));
            repeat(
                n,
                node,
                () -> {
                  read(ready, time);
                  read(due, time);
                });

            check(node.get(ready).le(node.get(due)), "the time window of a node is invalid");
            check(node.list().at(0).get(ready).eq(0), "the depot (node 0) must open at time 0");
          });

  private BuildTsptw() {}

  public static void main(String[] args) {
    Data data = spec.read(Path.of("formats/tsptw/cases/three-nodes/input.txt"));
    System.out.println(data.get(n) + " nodes");
    List<long[]> distances = data.get(dist);
    long[] opens = data.get(ready);
    long[] closes = data.get(due);
    for (int k = 0; k < distances.size(); k++) {
      System.out.printf(
          "node %d: open %d..%d, distances %s%n",
          k, opens[k], closes[k], Arrays.toString(distances.get(k)));
    }
  }
}
