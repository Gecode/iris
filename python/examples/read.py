import gecode_iris as iris

spec = iris.Spec.parse("n:int row[n]:int", origin="example.iris")
data = spec.read(b"3 10 20 30", origin="input.txt")

print(data["n"])
print(data.ints("row"))
print(data["row"])
