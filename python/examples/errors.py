import gecode_iris as iris

spec = iris.Spec.load("formats/dimacs-cnf/dimacs-cnf.iris")
try:
    spec.read("formats/dimacs-cnf/cases/literal-out-of-range/input.txt")
except iris.Error as error:
    print(error.code, error.origin, error.line, error.column)
    print(error.message)
    print(error)
