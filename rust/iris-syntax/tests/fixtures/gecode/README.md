# Gecode example reader specs

These are byte-for-byte `instance_spec` raw-string payloads from Gecode commit
`5e7bbd5c26b0702c6fd652356b2289da0710ac3c`, the Iris 0.1 reference.

| Fixture | Upstream source | Bytes |
|---|---|---:|
| sat.iris | examples/sat.cpp | 237 |
| tsp.iris | examples/tsp.cpp | 1389 |
| tsptw.iris | examples/tsptw.cpp | 306 |
| job-shop.iris | examples/job-shop.cpp | 631 |
| nonogram.iris | examples/nonogram.cpp | 767 |
| sudoku.iris | examples/sudoku-instances.hh | 297 |

From the repository root, regenerate or verify them with:

```sh
python3 rust/iris-syntax/tests/extract-gecode.py --reference-repo /path/to/gecode
python3 rust/iris-syntax/tests/extract-gecode.py --reference-repo /path/to/gecode --check
```

The script reads the pinned commit with `git show`; it never reads or changes
that repository's working tree. Without an explicit path it uses the cached
reference repository. It accepts exactly one empty-delimiter C++ raw string per
source file and copies its payload without decoding escapes or changing whitespace.
Normal Cargo tests use these committed fixtures and need no Gecode checkout.

## Upstream copyright and permission

Copyright notices from the six source files:

- sat.cpp: Raphael Reischuk, 2008; Guido Tack, 2008;
  Mikael Zayenz Lagerkvist, 2026.
- tsp.cpp: Christian Schulte, 2007.
- tsptw.cpp: Mikael Zayenz Lagerkvist, 2026.
- job-shop.cpp: Christian Schulte, 2019.
- nonogram.cpp: Mikael Zayenz Lagerkvist, 2005.
- sudoku-instances.hh: Mikael Zayenz Lagerkvist, 2005; Guido Tack, 2005;
  Christian Schulte, 2005.

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
"Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
