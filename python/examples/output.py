import gecode_iris as iris

spec = iris.Spec.load("formats/dimacs-cnf/dimacs-cnf.iris")
data = spec.read("formats/dimacs-cnf/cases/three-variables/input.txt")

print(data.to_dzn(), end="")
print(data.to_json(), end="")
