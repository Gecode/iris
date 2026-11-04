"""Read text instance files with Iris reader specs.

Parse a reader spec with ``Spec.parse`` or ``Spec.load``, then read an
input with ``spec.read``. Language errors raise ``Error``.

The package also installs the ``iris`` command, the same command line as the
Rust ``iris`` binary.
"""

from gecode_iris._data import Entry, Value
from gecode_iris._error import Error
from gecode_iris._native import Data, Spec

__all__ = ["Data", "Entry", "Error", "Spec", "Value"]
