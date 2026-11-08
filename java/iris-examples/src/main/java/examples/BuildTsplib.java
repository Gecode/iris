package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.FloatField;
import dev.gecode.iris.build.Gecode;
import dev.gecode.iris.build.IntExpr;
import dev.gecode.iris.build.IntField;
import dev.gecode.iris.build.RecordName;
import dev.gecode.iris.build.StringField;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * A TSPLIB reader. A file gives either an explicit distance matrix or node coordinates, from which
 * the distances follow.
 */
public final class BuildTsplib {
  /** The entries of one section of numbered coordinates, whose names start with a prefix. */
  public record Coordinates(
      IntField<long @Nullable []> node,
      FloatField<double @Nullable []> x,
      FloatField<double @Nullable []> y) {
    Coordinates(String prefix) {
      this(
          int1(prefix + "node").optional(),
          float1(prefix + "x").optional(),
          float1(prefix + "y").optional());
    }
  }

  public static final StringField<@Nullable String> name = string0("name").optional();
  public static final StringField<@Nullable String> type = string0("type").optional();
  public static final IntField<Long> n = int0("n");
  public static final StringField<@Nullable String> edgeWeightType =
      string0("edge_weight_type").optional();
  public static final StringField<@Nullable String> edgeWeightFormat =
      string0("edge_weight_format").optional();
  public static final StringField<@Nullable String> nodeCoordType =
      string0("node_coord_type").optional();
  public static final StringField<@Nullable String> displayDataType =
      string0("display_data_type").optional();
  public static final Coordinates coordinates = new Coordinates("");
  public static final IntField<@Nullable List<long[]>> weight = int2("weight").optional();
  public static final Coordinates display = new Coordinates("display_");

  private static final RecordName nodes = recordName("_", "nodes");

  public static final Spec spec =
      spec(
          "tsplib",
          () -> {
            comment("COMMENT");
            sections(
                until("EOF"),
                () -> {
                  arm(pattern("NAME : ", capture(name, rest())), optional());
                  arm(pattern("TYPE : ", capture(type, word())), optional());
                  arm(pattern("DIMENSION : ", capture(n, range(2).to(46340))));
                  arm(pattern("EDGE_WEIGHT_TYPE : ", capture(edgeWeightType, word())), optional());
                  arm(
                      pattern("EDGE_WEIGHT_FORMAT : ", capture(edgeWeightFormat, word())),
                      optional());
                  arm(pattern("NODE_COORD_TYPE : ", capture(nodeCoordType, word())), optional());
                  arm(
                      pattern("DISPLAY_DATA_TYPE : ", capture(displayDataType, word())),
                      optional());
                  coordinateSection("NODE_COORD_SECTION", n, nodes, coordinates);
                  arm(
                      "EDGE_WEIGHT_SECTION",
                      optional(),
                      () -> {
                        ifThen(
                            defined(edgeWeightFormat).not(),
                            () -> set(edgeWeightFormat, "FULL_MATRIX"));
                        var bound = Gecode.intMax.div(n);
                        read(weight, range(0).to(bound).dim(n).dim(n).layout(edgeWeightFormat));
                      });
                  coordinateSection("DISPLAY_DATA_SECTION", n, recordName(), display);
                });
            ifThen(defined(edgeWeightType).not(), () -> set(edgeWeightType, "EXPLICIT"));

            var x = coordinates.x();
            var y = coordinates.y();
            ifThen(
                edgeWeightType.eq("EXPLICIT"),
                () -> {
                  check(defined(weight), "no EDGE_WEIGHT_SECTION");
                  check(size(weight).eq(n), "edge weights do not match DIMENSION");
                });
            otherwise(() -> {
              check(
                  oneOf(edgeWeightType, "EUC_2D", "CEIL_2D", "ATT", "GEO"),
                  "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO");
              check(defined(x), "no NODE_COORD_SECTION");
              check(size(nodes.list()).eq(n), "coordinates do not match DIMENSION");
              check(
                  x.ge(-1e9).and(x.le(1e9)).and(y.ge(-1e9)).and(y.le(1e9)),
                  "coordinates must be between -1e9 and 1e9");
            });
          });

  private BuildTsplib() {}

  /** Adds the optional section {@code title} of {@code n} lines {@code node x y}. */
  static void coordinateSection(String title, IntField<Long> n, RecordName lines, Coordinates c) {
    arm(
        title,
        optional(),
        () ->
            repeat(
                n,
                lines,
                () -> {
                  read(c.node(), intType());
                  read(c.x(), floatType());
                  read(c.y(), floatType());
                }));
  }

  /** Whether {@code entry} is one of {@code values}: {@code entry == "A" || entry == "B"}. */
  static IntExpr oneOf(StringField<?> entry, String first, String... rest) {
    IntExpr any = entry.eq(first);
    for (String value : rest) {
      any = any.or(entry.eq(value));
    }
    return any;
  }

  public static void main(String[] args) {
    for (String example : List.of("explicit-five", "euc-2d-five")) {
      Data data = spec.read(Path.of("formats/tsplib/cases/" + example + "/input.txt"));
      String problem = data.get(name);
      System.out.printf(
          "%s: %d cities, %s%n",
          problem == null ? "unnamed" : problem, data.get(n), data.get(edgeWeightType));
      List<long[]> weights = data.get(weight);
      if (weights != null) {
        for (long[] row : weights) {
          System.out.println("  " + Arrays.toString(row));
        }
      }
      double[] x = data.get(coordinates.x());
      double[] y = data.get(coordinates.y());
      if (x != null && y != null) {
        System.out.println("  x " + Arrays.toString(x));
        System.out.println("  y " + Arrays.toString(y));
      }
    }
  }
}
