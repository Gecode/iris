# Using a reader spec from a program

Load this when the task is to use a spec from Gecode or another host
program, rather than to write the spec. Only the APIs below exist today;
do not guess others.

## Any language: the `iris` CLI

`iris read --json SPEC FILE` prints canonical JSON:

```json
{"iris":"0.1","entries":[
  {"name":"n","type":"int","depth":0,"value":3},
  {"name":"a","type":"int","depth":2,"value":[[1,2,3],[9]]}
]}
```

Entries come in definition order. `type` is `int`, `float`, or `string`;
`depth` is 0 for a scalar and the number of array levels otherwise. Float
values may be written without a decimal point (`1`, `3e2`), so use `type`,
not the spelling, to tell floats from integers. Exit status 0 means success;
1 means a language or file error (message on stderr); 2 means a usage
error. `--dzn` prints MiniZinc data instead, and `--param NAME=VALUE`
supplies parameters, including `instance=N` or `instance=NAME`.

## Gecode 6.5.0 (C++)

Gecode's instance module (`#include <gecode/instance.hh>`, namespace
`Gecode::Instance`) implements Iris 0.1; the language differences are listed
in appendix C of the Specification. A script embeds its spec as a raw string
and reads the instance file named on the command line through
`InstanceOptions` (`#include <gecode/driver.hh>`):

```cpp
const char* instance_spec = R"(format dimacs-cnf
comment "c"
line "p cnf {vars:0..gecode.int_max} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0
}
)";

int main(int argc, char* argv[]) {
  InstanceOptions opt("SAT", instance_spec);
  opt.parse(argc, argv);
  try {
    (void) opt.data();  // read now, to report errors before solving
  } catch (const Instance::Error& e) {
    std::cerr << e.what() << std::endl;
    return 1;
  }
  Script::run<Sat, DFS, InstanceOptions>(opt);
  return 0;
}
```

In the script's constructor:

```cpp
Instance::Data d = opt.data();
int vars = static_cast<int>(d.integer("vars"));
for (int c = 0; c < d.size("clause"); c++) {
  IntArgs literals = d.ints("clause", {c});  // row c of a depth-2 entry
  // ...
}
```

Such a script accepts `-spec FILE` to read with a replacement spec, which
must define the same data, and `-param NAME=VALUE` (repeat it, or separate
pairs by commas) for spec parameters such as `instance=3`.

The API, from `gecode/instance.hh`:

| API | Purpose |
|---|---|
| `Spec::parse(text, origin)`, `Spec::load(path)` | Parse a spec; throws `Instance::Error`. |
| `spec.read(path, params)`, `spec.read_text(text, origin, params)` | Read an instance into `Data`. |
| `spec.catalog(path, params)` | List instances: `instances`, `name` (if all have one), `failed`. |
| `spec.name()` | The `format` name. |
| `Params p; p.set("instance", "2");` | Parameter values, as strings. |
| `d.has(n)`, `d.type(n)`, `d.depth(n)`, `d.entries()`, `d.name(i)` | Inspect entries. |
| `d.integer(n, path)`, `d.number(n, path)`, `d.text(n, path)` | One value; `path` is an `IntArgs` of indices from 0. |
| `d.ints(n, path)`, `d.numbers(n, path)`, `d.size(n, path)` | A whole array or row, and its length. |
| `d.dzn(os)` | Print as MiniZinc data. |

Accessors throw `Instance::Error` for an unknown name, a wrong type or
depth, an index out of range, or an integer that does not fit in `int`.
This is why Gecode specs bound values by `gecode.int_max`.

The tool `gecode-instance -spec SPEC [-summary|-dzn|-list] [-param N=V]
FILE...` reads files like `iris read`.

## Rust

The Iris repository's `rust/gecode-iris` crate (not yet published) is the
reference implementation's API:

```rust
use gecode_iris::{Params, Spec};

let spec = Spec::parse(b"n:int row[n]:int", "example.iris")?;
let data = spec.read(b"3 10 20 30", "input.txt", &Params::new())?;
assert_eq!(data.int("n")?, 3);
assert_eq!(data.ints("row", &[])?, &[10, 20, 30]);
```

## Python, Java, Kotlin, standalone C++

Planned (Python binds the Rust crate; C++ and Java are native
implementations), but not available yet. Until they exist, run the `iris`
CLI and parse its JSON.
