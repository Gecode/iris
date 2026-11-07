"""Gecode's host constants, for ranges and checks."""

from gecode_iris.build._expr import IntExpr, host

__all__ = ["INT_MAX", "INT_MIN"]

INT_MAX: IntExpr = host("gecode.int_max")
"""``gecode.int_max``, 2147483646."""

INT_MIN: IntExpr = host("gecode.int_min")
"""``gecode.int_min``, -2147483646."""
