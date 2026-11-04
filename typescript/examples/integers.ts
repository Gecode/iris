import { Spec } from "gecode-iris";

const spec = Spec.parse("big:int row[2]:int");
const data = spec.read("9007199254740993 1 -9223372036854775808");

try {
  data.int("big");
} catch (error) {
  if (!(error instanceof RangeError)) throw error;
  console.log(error.message);
}
console.log(data.bigInt("big"));
console.log(data.bigInts("row"));
console.log(data.toObject({ bigint: true }));
