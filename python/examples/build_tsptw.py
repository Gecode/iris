from __future__ import annotations

from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class Tsptw(NamedTuple):
    """A reader for the travelling salesman problem with time windows: a
    distance matrix and a time window for each node, with the depot as node 0."""

    spec: iris.Spec
    n: ib.IntField[int]
    dist: ib.IntField[list[list[int]]]
    ready: ib.IntField[list[int]]
    due: ib.IntField[list[int]]


def tsptw() -> Tsptw:
    b = ib.SpecBuilder("tsptw")
    b.comment("#")
    n = b.read("n", ib.int_range(2, 1000))
    dist = b.read("dist", ib.int_range(0, ib.div(ib.div(ib.gecode.INT_MAX, 2), n)).dims(n, n))

    time = b.define_type("time", ib.int_range(0, ib.div(ib.gecode.INT_MAX, 2)))
    with b.repeat(n) as node:
        node.as_name("nodes")
        ready = node.read("ready", time)
        due = node.read("due", time)
    nodes = node.record

    b.check(nodes[ready] <= nodes[due], "the time window of a node is invalid")
    b.check(nodes.list[0][ready] == 0, "the depot (node 0) must open at time 0")
    return Tsptw(b.build(), n, dist, ready, due)


def main() -> None:
    problem = tsptw()
    data = problem.spec.read("formats/tsptw/cases/three-nodes/input.txt")
    print(f"{data[problem.n]} nodes")
    windows = zip(data[problem.ready], data[problem.due], strict=True)
    for node, (distances, (ready, due)) in enumerate(zip(data[problem.dist], windows, strict=True)):
        print(f"node {node}: open {ready}..{due}, distances {distances}")


if __name__ == "__main__":
    main()
