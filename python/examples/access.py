import gecode_iris as iris

spec = iris.Spec.load("formats/dimacs-cnf/dimacs-cnf.iris")
data = spec.read("formats/dimacs-cnf/cases/three-variables/input.txt")

for entry in data.entries:
    print(entry)
print("clause" in data, "model" in data)
print(data["clause"])
print(data.ints("clause", -1))
print(data.to_dict())
