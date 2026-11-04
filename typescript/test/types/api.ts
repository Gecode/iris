// Type tests for the published declarations, checked by `npm run test:types`
// and never run. Each @ts-expect-error line must report an error.
import {
  Data,
  initialize,
  IrisError,
  IrisInternalError,
  Spec,
  type BigIntValue,
  type Entry,
  type Location,
  type ModuleSource,
  type Params,
  type Value,
  type ValueType,
} from "gecode-iris";
import { Spec as InlineSpec } from "gecode-iris/inline";
import { initialize as initializeManually, Spec as ManualSpec } from "gecode-iris/manual";

export function validUsage(input: Uint8Array): void {
  using spec = Spec.parse("n: int  a[n]: int", "counted.iris");
  const pairs: Params = [["instance", "2"]];
  const data: Data = spec.read(input, "input.txt", { params: { instance: "1" } });
  spec.catalog("text", undefined, { params: pairs });
  spec.read("1 1");
  const entries: readonly Entry[] = data.entries;
  const valueType: ValueType | undefined = data.entry("a")?.valueType;
  const present: boolean = data.has("a");
  const count: number = data.int("n") + data.size("a") + data.float("f", [0]);
  const exact: bigint = data.bigInt("n");
  const row: number[] = data.ints("a", [0]);
  const exactRow: BigInt64Array = data.bigInts("a");
  const words: string[] = data.strings("w");
  const value: Value = data.value("a");
  const exactValue: BigIntValue = data.value("a", [], { bigint: true });
  const object: Record<string, Value> = data.toObject();
  const exactObject: Record<string, BigIntValue> = data.toObject({ bigint: true });
  const documents: string = data.toJSON() + data.toDzn();
  void [entries, valueType, present, count, exact, row, exactRow, words, value];
  void [exactValue, object, exactObject, documents];
}

export async function loading(bytes: Uint8Array, response: Response): Promise<void> {
  const source: ModuleSource = new URL("https://example.com/iris_js_bg.wasm");
  await initialize(source);
  await initializeManually(fetch("/iris_js_bg.wasm"));
  await initializeManually(bytes);
  await initializeManually(response);
  const specs: Spec[] = [InlineSpec.parse("n: int"), ManualSpec.parse("n: int")];
  void specs;
}

export function errorFields(error: unknown): void {
  if (error instanceof IrisError) {
    const code: string = error.code;
    const location: Location | null = error.inputLocation ?? error.specLocation;
    const line: number | null | undefined = location?.line;
    const path: readonly number[] | null = error.recordPath;
    const text: string = error.diagnostic + error.toJSON() + error.message;
    void [code, line, path, text, error.context];
  }
  if (error instanceof IrisInternalError) {
    const cause: unknown = error.cause;
    const name: "IrisInternalError" = error.name;
    void [cause, name, error.message];
  }
}

export function misuse(data: Data, error: IrisError, entry: Entry): void {
  // @ts-expect-error -- the constructor is private
  const spec = new Spec();
  // @ts-expect-error -- IrisError is never constructed by users
  const constructed = new IrisError("message");
  // @ts-expect-error -- Data comes from read and catalog only
  const empty = new Data(new Uint8Array());
  // @ts-expect-error -- spec text is a string or bytes
  Spec.parse(42);
  // @ts-expect-error -- initialize needs a source
  void initialize();
  // @ts-expect-error -- parameter values are strings
  Spec.parse("").read("", "input.txt", { params: { n: 3 } });
  // @ts-expect-error -- entry names are strings
  data.int(1);
  // @ts-expect-error -- a path is an array of indices
  data.ints("a", "0");
  // @ts-expect-error -- bigint values are not numbers
  const narrow: number = data.value("n", [], { bigint: true });
  // @ts-expect-error -- fields are readonly
  error.code = "E-OTHER";
  // @ts-expect-error -- entries are readonly
  data.entries.push(entry);
  // @ts-expect-error -- bool is not a value type
  if (entry.valueType === "bool") {
    return;
  }
  void [spec, constructed, empty, narrow];
}
