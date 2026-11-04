import gecode_iris as iris

spec = iris.Spec.load("formats/jobshop/jobshop.iris")
path = "formats/jobshop/cases/or-library/input.txt"

catalog = spec.catalog(path)
print(catalog.to_dict())

data = spec.read(path, params={"instance": "handmade-or"})
print(data["n_jobs"], data["n_machines"])
