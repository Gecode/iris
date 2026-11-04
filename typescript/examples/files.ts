import { readFileSync } from "node:fs";
import { Spec } from "gecode-iris";

const specPath = "formats/jobshop/jobshop.iris";
const inputPath = "formats/jobshop/cases/or-library/input.txt";
const spec = Spec.parse(readFileSync(specPath), specPath);
const input = readFileSync(inputPath);

const catalog = spec.catalog(input, inputPath);
console.log(catalog.toObject());

const data = spec.read(input, inputPath, { params: { instance: "handmade-or" } });
console.log(data.int("n_jobs"), data.int("n_machines"));
