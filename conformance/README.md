# conformance

This directory will hold the conformance suite, the contract between
implementations. `cases/<version>/<area>/<case>/` holds one case per
directory, `runner/` runs the cases against any implementation's driver,
and `adapters/gecode/` is a driver linked against Gecode's
`libgecodeinstance`, so the suite also runs against Gecode 6.5.0.
`PLAN.md` §7 describes the suite; milestone M2 builds it.
