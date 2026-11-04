import { Worker } from "node:worker_threads";

function readWithDeadline(spec: string, input: string, milliseconds: number): Promise<string> {
  const worker = new Worker(new URL("./deadline-worker.ts", import.meta.url), {
    workerData: { spec, input },
  });
  const timer = setTimeout(() => void worker.terminate(), milliseconds);
  return new Promise<string>((resolve, reject) => {
    worker.once("message", resolve);
    worker.once("error", reject);
    worker.once("exit", () => reject(new Error("read stopped at the deadline")));
  }).finally(() => clearTimeout(timer));
}

process.stdout.write(await readWithDeadline("n:int row[n]:int", "2 5 7", 5000));
