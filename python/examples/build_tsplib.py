from __future__ import annotations

from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class Tsplib(NamedTuple):
    """A TSPLIB reader. A file gives either an explicit distance matrix or node
    coordinates, from which the distances follow."""

    spec: iris.Spec
    name: ib.StrField[str | None]
    n: ib.IntField[int]
    edge_weight_type: ib.StrField[str | None]
    weight: ib.IntField[list[list[int]] | None]
    x: ib.FloatField[list[float] | None]
    y: ib.FloatField[list[float] | None]


class Coordinates(NamedTuple):
    """The entries of one section of numbered coordinates."""

    nodes: ib.Record
    x: ib.FloatField[list[float] | None]
    y: ib.FloatField[list[float] | None]


def one_of(entry: ib.StrField[str | None], *values: str) -> ib.IntExpr:
    """Whether ``entry`` is one of ``values``: ``entry == "A" || entry == "B"``."""
    first, *rest = values
    test = entry == first
    for value in rest:
        test = test | (entry == value)
    return test


Sections = ib.Arms[ib.Scope0[ib.Required], ib.Scope0[None]]


@ib.track_caller
def coordinate_section(arms: Sections, title: str, n: ib.IntField[int], prefix: str) -> Coordinates:
    """Adds the optional section ``title`` of ``n`` lines ``node x y``, with the
    entries named ``{prefix}node``, ``{prefix}x`` and ``{prefix}y``."""
    with arms.arm(title, optional=True) as a, a.repeat(n) as line:
        line.read(f"{prefix}node", ib.int_)
        x = line.read(f"{prefix}x", ib.float_)
        y = line.read(f"{prefix}y", ib.float_)
    return Coordinates(line.record, x, y)


def tsplib() -> Tsplib:
    b = ib.SpecBuilder("tsplib")
    b.comment("COMMENT")
    name = b.optional_capture("name", ib.rest)
    problem_type = b.optional_capture("type", ib.word)
    n = b.capture("n", ib.int_range(2, 46340))
    edge_weight_type = b.optional_capture("edge_weight_type", ib.word)
    edge_weight_format = b.optional_capture("edge_weight_format", ib.word)
    node_coord_type = b.optional_capture("node_coord_type", ib.word)
    display_data_type = b.optional_capture("display_data_type", ib.word)
    with b.sections(until="EOF") as s:
        s.arm(("NAME : ", name), optional=True)
        s.arm(("TYPE : ", problem_type), optional=True)
        s.arm(("DIMENSION : ", n))
        s.arm(("EDGE_WEIGHT_TYPE : ", edge_weight_type), optional=True)
        s.arm(("EDGE_WEIGHT_FORMAT : ", edge_weight_format), optional=True)
        s.arm(("NODE_COORD_TYPE : ", node_coord_type), optional=True)
        s.arm(("DISPLAY_DATA_TYPE : ", display_data_type), optional=True)
        coordinates = coordinate_section(s, "NODE_COORD_SECTION", n, "")
        with s.arm("EDGE_WEIGHT_SECTION", optional=True) as a:
            with a.if_(~ib.defined(edge_weight_format)) as t:
                t.set(edge_weight_format, "FULL_MATRIX")
            bound = ib.div(ib.gecode.INT_MAX, n)
            weights = ib.int_range(0, bound).dims(n, n).layout(edge_weight_format)
            weight = a.read("weight", weights)
        coordinate_section(s, "DISPLAY_DATA_SECTION", n, "display_")
    with b.if_(~ib.defined(edge_weight_type)) as t:
        t.set(edge_weight_type, "EXPLICIT")

    x, y = coordinates.x, coordinates.y
    with b.if_(edge_weight_type == "EXPLICIT") as t:
        t.check(ib.defined(weight), "no EDGE_WEIGHT_SECTION")
        t.check(ib.size(weight) == n, "edge weights do not match DIMENSION")
    with b.else_() as e:
        e.check(
            one_of(edge_weight_type, "EUC_2D", "CEIL_2D", "ATT", "GEO"),
            "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO",
        )
        e.check(ib.defined(x), "no NODE_COORD_SECTION")
        e.check(ib.size(coordinates.nodes) == n, "coordinates do not match DIMENSION")
        e.check(
            (x >= -1e9) & (x <= 1e9) & (y >= -1e9) & (y <= 1e9),
            "coordinates must be between -1e9 and 1e9",
        )
    return Tsplib(b.build(), name, n, edge_weight_type, weight, x, y)


def main() -> None:
    problem = tsplib()
    for case in ["explicit-five", "euc-2d-five"]:
        data = problem.spec.read(f"formats/tsplib/cases/{case}/input.txt")
        name = data[problem.name] or "unnamed"
        weight_type = data[problem.edge_weight_type] or "?"
        print(f"{name}: {data[problem.n]} cities, {weight_type}")
        weight = data[problem.weight]
        if weight is not None:
            for row in weight:
                print(f"  {row}")
        x, y = data[problem.x], data[problem.y]
        if x is not None and y is not None:
            print(f"  x {x}\n  y {y}")


if __name__ == "__main__":
    main()
