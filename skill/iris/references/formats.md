# Format library index

The Iris repository's `formats/` directory holds ready-made Iris 0.1 reader
specs for six benchmark formats, copied from Gecode 6.5.0's examples, each
with a README and test cases (`formats/<name>/<name>.iris` and
`formats/<name>/cases/*/input.txt`). Before writing a spec, look here for a
format with a similar shape and adapt its spec. The specs below are shown
as `iris fmt` formats them, each with a sample input and the data read.

| Format | Shape | Constructs worth copying |
|---|---|---|
| [DIMACS CNF](#dimacs-cnf) | Comment lines, a `p cnf` header, zero-terminated clauses | `comment`, `stop`, header `line`, sentinel `list` in a counted repeat |
| [TSPLIB](#tsplib) | `KEY : VALUE` header lines in any order, then data sections, `EOF` | `sections` with `optional` arms, layouts, `defined`, defaults with `let` |
| [TSPTW](#tsptw) | Count, square matrix, one record per node | `type` for a shared range, `as`/`each`, record checks |
| [Job shop](#job-shop) | Free text, a counts line, nested records; several instances | `instances`, `sections skip other` with a `last` arm to skip prose, nested repeats, `distinct` per record |
| [Sudoku](#sudoku) | `# size=N box=WxH` header, character grid; several puzzles | header pattern, `alphabet` with `blank`, `instances` |
| [Nonogram](#nonogram) | Keyword lines, comma-separated clue rows | `separators ","`, `list ... until eol`, ragged rows, row checks |

The data from a spec in this library is what the corresponding Gecode
example reads, so a spec that replaces one must define the same names.

## DIMACS CNF

Reads the header and the declared number of clauses. Each clause is a list
ended by `0`, which is not stored. `stop "%"` ends the input at a SATLIB-style
`%` line, and `c` lines are comments.

```iris
format dimacs-cnf

comment "c"
stop "%" # SATLIB files end with "%" and "0"
line "p cnf {vars:0..gecode.int_max} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0 # the terminating 0 is not stored
}
```

```input
c Hand-made three-variable formula
p cnf 3 3
1 -2 0
2 3 -1 0
-3 0
```

```data
vars = 3
clauses = 3
clause = [[1, -2], [2, 3, -1], [-3]]
```

## TSPLIB

A TSP reader for explicit weight matrices in any TSPLIB layout and for
coordinates. The header keys may come in any order; each data section reads
the following lines. The `EOF` terminator is optional. `COMMENT` lines are
skipped by the `comment` setting. Note how `let` supplies a default for an
optional key and how `layout` takes the layout name from the input.

```iris
format tsplib

comment "COMMENT"
sections until "EOF" {
  "NAME : {name:rest}" optional
  "TYPE : {type:word}" optional
  "DIMENSION : {n:2..46340}"
  "EDGE_WEIGHT_TYPE : {edge_weight_type:word}" optional
  "EDGE_WEIGHT_FORMAT : {edge_weight_format:word}" optional
  "NODE_COORD_TYPE : {node_coord_type:word}" optional
  "DISPLAY_DATA_TYPE : {display_data_type:word}" optional
  "NODE_COORD_SECTION" optional {
    repeat n as nodes {
      node: int
      x: float
      y: float
    }
  }
  "EDGE_WEIGHT_SECTION" optional {
    if !defined(edge_weight_format) {
      let edge_weight_format = "FULL_MATRIX"
    }
    weight[n][n]: 0..gecode.int_max / n layout edge_weight_format
  }
  "DISPLAY_DATA_SECTION" optional {
    repeat n {
      display_node: int
      display_x: float
      display_y: float
    }
  }
}
if !defined(edge_weight_type) {
  let edge_weight_type = "EXPLICIT"
}

if edge_weight_type == "EXPLICIT" {
  check defined(weight) "no EDGE_WEIGHT_SECTION"
  check size(weight) == n "edge weights do not match DIMENSION"
} else {
  check edge_weight_type == "EUC_2D" ||
    edge_weight_type == "CEIL_2D" ||
    edge_weight_type == "ATT" ||
    edge_weight_type == "GEO"
    "EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO"
  check defined(x) "no NODE_COORD_SECTION"
  check size(nodes) == n "coordinates do not match DIMENSION"
  check x >= -1e9 && x <= 1e9 && y >= -1e9 && y <= 1e9 "coordinates must be between -1e9 and 1e9"
}
```

```input
NAME : handmade-euc-2d-five
TYPE : TSP
DIMENSION : 5
EDGE_WEIGHT_TYPE : EUC_2D
NODE_COORD_TYPE : TWOD_COORDS
NODE_COORD_SECTION
1 0 0
2 3 0
3 3 4
4 0 4
5 1.5 2
EOF
```

```data
name = "handmade-euc-2d-five"
type = "TSP"
n = 5
edge_weight_type = "EUC_2D"
node_coord_type = "TWOD_COORDS"
node = [1, 2, 3, 4, 5]
x = [0.0, 3.0, 3.0, 0.0, 1.5]
y = [0.0, 0.0, 4.0, 4.0, 2.0]
```

## TSPTW

A count, an `n` by `n` travel-time matrix, then a time window per node. The
named type `time` bounds both fields; the record checks name the failing
node.

```iris
format tsptw

comment "#"
n: 2..1000
dist[n][n]: 0..gecode.int_max / 2 / n

type time = 0..gecode.int_max / 2
repeat n as nodes {
  each node
  ready: time
  due: time
}

check node.ready <= node.due "the time window of a node is invalid"
check nodes[0].ready == 0 "the depot (node 0) must open at time 0"
```

```input
# Hand-made depot and two customers
3
0 2 4
3 0 2
4 3 0
0 20
2 8
5 12
```

```data
n = 3
dist = [[0, 2, 4], [3, 0, 2], [4, 3, 0]]
ready = [0, 2, 5]
due = [20, 8, 12]
```

## Job shop

OR-Library and JSPLIB job shop files. `sections skip other` skips free text
until the line with the two counts; that arm is `last`, and its body reads
the jobs. `instances` lets one file hold several instances, selected by
number or by the `instance NAME` line. `check distinct(job.machine)` runs
once per job.

```iris
format jobshop

comment "#" "+++"
instances {
  sections skip other {
    "instance {name:word}" optional
    "{n_jobs:1..} {n_machines:1..}" last {
      check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"

      repeat n_jobs as jobs {
        each job
        repeat n_machines as steps {
          each step
          machine: 0..<n_machines
          duration: 0..
        }
      }
    }
  }

  check distinct(job.machine) "a job uses a machine twice; each job must use every machine once"
  check sum(j in jobs: sum(j.duration)) <= gecode.int_max "the total duration is too large"
}
```

```input instance=handmade-or
Hand-made OR-Library style file
+++++++++++++++++++++++++++++
instance handmade-or
+++++++++++++++++++++++++++++
Three jobs on three machines
3 3
0 2 1 3 2 1
1 1 2 4 0 2
2 3 0 1 1 2
```

```data
name = "handmade-or"
n_jobs = 3
n_machines = 3
machine = [[0, 1, 2], [1, 2, 0], [2, 0, 1]]
duration = [[2, 3, 1], [1, 4, 2], [3, 1, 2]]
```

## Sudoku

A header line with the size and box shape, then an `n` by `n` grid. The
alphabet maps `1`–`9` and `A`–`Z` to 1–35 (and `0` to 36), and `.` to 0.

```iris
format sudoku

instances {
  line "# size={n:1..} box={box_width:1..}x{box_height:1..}"
  cells[n][n]: alphabet "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0" blank "."

  check box_width * box_height == n "boxes must tile the grid"
  check max(cells) <= n "a value is larger than the size of the grid"
}
```

```input
# size=4 box=2x2
1..4
.41.
.14.
4..1
```

```data
n = 4
box_width = 2
box_height = 2
cells = [[1, 0, 0, 4], [0, 4, 1, 0], [0, 1, 4, 0], [4, 0, 0, 1]]
```

## Nonogram

Keyword lines in any order. Clues are comma-separated, one row or column per
line, so `separators ","` plus `list ... until eol` gives ragged rows. A row
with no filled cells is written `0`, which keeps the line nonempty. The row
checks use `sum(row)` and `count(row)`, which in a check mean one row at a
time.

```iris
format nonogram

separators ","
sections skip other {
  "width {width:1..}"
  "height {height:1..}"
  "rows" {
    repeat height as rows {
      list row: 0.. until eol
    }
  }
  "columns" {
    repeat width as cols {
      list col: 0.. until eol
    }
  }
}

check height <= 1000000 / width "the board must have at most a million fields"
check size(rows) == height && size(cols) == width
  "the numbers of rows and columns differ from the board size"
check count(row) <= 1 || min(row) > 0 "a 0 must be the only clue of its row"
check count(col) <= 1 || min(col) > 0 "a 0 must be the only clue of its column"
check sum(row) + count(row) - 1 <= width "the clues of a row do not fit into the width of the board"
check sum(col) + count(col) - 1 <= height
  "the clues of a column do not fit into the height of the board"
```

```input
title "Hand-made X"
width 5
height 5
rows
1,1
1,1
1
1,1
1,1
columns
1,1
1,1
1
1,1
1,1
```

```data
width = 5
height = 5
row = [[1, 1], [1, 1], [1], [1, 1], [1, 1]]
col = [[1, 1], [1, 1], [1], [1, 1], [1, 1]]
```
