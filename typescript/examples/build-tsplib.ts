import { readFileSync } from "node:fs";
import {
  type Arms,
  defined,
  div,
  type Field,
  float,
  floatLit,
  gecode,
  int,
  p,
  range,
  rest,
  size,
  spec,
  trackCaller,
  word,
} from "gecode-iris/build";

/** Whether `entry` is one of `values`: `entry == "A" || entry == "B"`. */
function oneOf(entry: Field<"string", 0>, first: string, ...others: string[]) {
  return others.reduce((test, value) => test.or(entry.eq(value)), entry.eq(first));
}

/**
 * Adds the optional section `title` of `n` lines `node x y`, with the entries
 * named `${prefix}node`, `${prefix}x` and `${prefix}y`.
 */
const coordinateSection = trackCaller(
  (arms: Arms, title: string, n: Field<"int", 0>, prefix: string) =>
    arms.arm(title, { optional: true }, (a) =>
      a.repeat(n, (line) => {
        line.read(`${prefix}node`, int);
        const x = line.read(`${prefix}x`, float);
        const y = line.read(`${prefix}y`, float);
        return { nodes: line.record, x, y };
      }),
    ),
);

/**
 * Reads a TSPLIB file, which gives either an explicit distance matrix or node
 * coordinates, from which the distances follow.
 */
export function tsplib() {
  return spec("tsplib", (b) => {
    b.comment("COMMENT");
    const name = b.optionalCapture("name", rest);
    const problemType = b.optionalCapture("type", word);
    const n = b.capture("n", range(2, 46340));
    const edgeWeightType = b.optionalCapture("edge_weight_type", word);
    const edgeWeightFormat = b.optionalCapture("edge_weight_format", word);
    const nodeCoordType = b.optionalCapture("node_coord_type", word);
    const displayDataType = b.optionalCapture("display_data_type", word);
    const { coordinates, weight } = b.sections({ until: "EOF" }, (s) => {
      s.arm(p`NAME : ${name}`, { optional: true });
      s.arm(p`TYPE : ${problemType}`, { optional: true });
      s.arm(p`DIMENSION : ${n}`);
      s.arm(p`EDGE_WEIGHT_TYPE : ${edgeWeightType}`, { optional: true });
      s.arm(p`EDGE_WEIGHT_FORMAT : ${edgeWeightFormat}`, { optional: true });
      s.arm(p`NODE_COORD_TYPE : ${nodeCoordType}`, { optional: true });
      s.arm(p`DISPLAY_DATA_TYPE : ${displayDataType}`, { optional: true });
      const handles = {
        coordinates: coordinateSection(s, "NODE_COORD_SECTION", n, ""),
        weight: s.arm("EDGE_WEIGHT_SECTION", { optional: true }, (a) => {
          a.if(defined(edgeWeightFormat).not(), (t) => t.set(edgeWeightFormat, "FULL_MATRIX"));
          const weights = range(0, div(gecode.intMax, n)).dims(n, n).layout(edgeWeightFormat);
          return a.read("weight", weights);
        }),
      };
      coordinateSection(s, "DISPLAY_DATA_SECTION", n, "display_");
      return handles;
    });
    b.if(defined(edgeWeightType).not(), (t) => t.set(edgeWeightType, "EXPLICIT"));

    const { x, y } = coordinates;
    const limit = floatLit(1e9);
    b.if(edgeWeightType.eq("EXPLICIT"), (t) => {
      t.check(defined(weight), "no EDGE_WEIGHT_SECTION");
      t.check(size(weight).eq(n), "edge weights do not match DIMENSION");
    }).else((e) => {
      e.check(
        oneOf(edgeWeightType, "EUC_2D", "CEIL_2D", "ATT", "GEO"),
        "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO",
      );
      e.check(defined(x), "no NODE_COORD_SECTION");
      e.check(size(coordinates.nodes).eq(n), "coordinates do not match DIMENSION");
      e.check(
        x.ge(limit.neg()).and(x.le(limit)).and(y.ge(limit.neg())).and(y.le(limit)),
        "coordinates must be between -1e9 and 1e9",
      );
    });
    return { name, n, edgeWeightType, weight, x, y };
  });
}

if (import.meta.main) {
  const problem = tsplib();
  for (const example of ["explicit-five", "euc-2d-five"]) {
    const data = problem.read(readFileSync(`formats/tsplib/cases/${example}/input.txt`));
    console.log(`${data.name ?? "unnamed"}: ${data.n} cities, ${data.edgeWeightType ?? "?"}`);
    for (const row of data.weight ?? []) {
      console.log(`  ${row.join(" ")}`);
    }
    if (data.x !== undefined && data.y !== undefined) {
      console.log(`  x ${data.x.join(" ")}\n  y ${data.y.join(" ")}`);
    }
  }
}
