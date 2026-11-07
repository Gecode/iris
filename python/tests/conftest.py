import sys

# t-strings are syntax from Python 3.14 on; older versions cannot parse these.
collect_ignore = [] if sys.version_info >= (3, 14) else ["test_build_tstrings.py"]
