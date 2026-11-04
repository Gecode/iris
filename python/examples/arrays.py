# Needs numpy.
import numpy as np

import gecode_iris as iris

spec = iris.Spec.load("formats/jobshop/jobshop.iris")
data = spec.read("formats/jobshop/cases/jsplib/input.txt")

duration = data.array("duration")
assert isinstance(duration, np.ndarray)  # a ragged array stays nested lists
print(duration.shape, duration.dtype)
print(duration.sum(axis=1))
print(data.array("duration", 0))
