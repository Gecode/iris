from collections.abc import Mapping, Sequence
from os import PathLike
from typing import Any, final

from numpy.typing import NDArray

from gecode_iris._data import Entry, Value

@final
class Spec:
    """A parsed and checked reader spec.

    One ``Spec`` can read any number of inputs, and threads can share it.
    """

    @staticmethod
    def parse(text: str | bytes | bytearray, origin: str = "<string>") -> Spec:
        """Parse and check a reader spec given as ``str`` or ``bytes``."""

    @staticmethod
    def load(path: str | PathLike[str]) -> Spec:
        """Read and parse a reader spec file; the path is the error origin."""

    def read(
        self,
        source: bytes | bytearray | str | PathLike[str],
        params: Mapping[str, str] | None = None,
        *,
        origin: str | None = None,
    ) -> Data:
        """Read one instance from input bytes or from a path."""

    def catalog(
        self,
        source: bytes | bytearray | str | PathLike[str],
        params: Mapping[str, str] | None = None,
        *,
        origin: str | None = None,
    ) -> Data:
        """List the instances in the input, with the same arguments as ``read``."""

@final
class Data:
    """The data from one read or catalog.

    ``data[name]`` returns an entry's value and ``name in data`` tests for an
    entry. Values are copied into Python objects on each access: an ``int``,
    ``float`` or ``str`` at depth 0, and nested lists at greater depths.
    """

    @property
    def entries(self) -> tuple[Entry, ...]:
        """The entries in the order their names were defined."""

    def __getitem__(self, name: str, /) -> Value:
        """The whole value of an entry; raises ``KeyError`` for an unknown name."""

    def __contains__(self, name: object, /) -> bool: ...
    def ints(self, name: str, /, *path: int) -> list[int]:
        """The integer row of an entry at a path."""

    def array(self, name: str, /, *path: int) -> NDArray[Any] | list[Value]:
        """An array of an entry, or of its row at a path.

        The result is a ``numpy.ndarray`` when numpy is importable and the
        array is rectangular, and nested lists otherwise.
        """

    def to_dict(self) -> dict[str, Value]:
        """All entries as a ``dict`` from name to value, in entry order."""

    def to_json(self) -> str:
        """The canonical JSON document, ending in a newline."""

    def to_dzn(self) -> str:
        """The MiniZinc data text defined in the Specification's appendix B."""

def run_cli(argv: Sequence[str]) -> int:
    """Run the ``iris`` command line on ``argv``, program name first; return its exit status."""
