import { parentPort, workerData } from "node:worker_threads";
import { Spec } from "gecode-iris";

const { spec, input } = workerData;
parentPort?.postMessage(Spec.parse(spec).read(input).toJSON());
