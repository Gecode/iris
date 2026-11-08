import { readFileSync } from "node:fs";
import { div, gecode, range, spec } from "gecode-iris/build";

/**
 * Reads a distance matrix and a time window for each node, with the depot as
 * node 0.
 */
export function tsptw() {
  return spec("tsptw", (b) => {
    b.comment("#");
    const n = b.read("n", range(2, 1000));
    const dist = b.read("dist", range(0, div(div(gecode.intMax, 2), n)).dims(n, n));

    const time = b.defineType("time", range(0, div(gecode.intMax, 2)));
    const { node, ready, due } = b.repeat(n, (r) => {
      r.asName("nodes");
      r.eachName("node");
      return { node: r.record, ready: r.read("ready", time), due: r.read("due", time) };
    });

    b.check(node.get(ready).le(node.get(due)), "the time window of a node is invalid");
    b.check(node.list.at(0).get(ready).eq(0), "the depot (node 0) must open at time 0");
    return { n, dist, ready, due };
  });
}

if (import.meta.main) {
  const data = tsptw().read(readFileSync("formats/tsptw/cases/three-nodes/input.txt"));
  console.log(`${data.n} nodes`);
  for (const [node, distances] of data.dist.entries()) {
    const window = `${data.ready[node]}..${data.due[node]}`;
    console.log(`node ${node}: open ${window}, distances ${distances.join(" ")}`);
  }
}
