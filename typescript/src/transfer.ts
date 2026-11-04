// Decodes the binary data layout documented in rust/iris-js/src/transfer.rs.
// Numeric arrays stay views of the buffer; strings are decoded once.

const FORMAT_VERSION = 1;
const VALUE_TYPES = ["int", "float", "string"] as const;
// Keeps a leading U+FEFF, which is part of the reader's string.
const utf8 = new TextDecoder("utf-8", { ignoreBOM: true });
// Typed arrays use the platform's byte order, and the layout is little-endian.
const littleEndian = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

export type ValueType = (typeof VALUE_TYPES)[number];

export type Leaves =
  | { readonly valueType: "int"; readonly values: BigInt64Array }
  | { readonly valueType: "float"; readonly values: Float64Array }
  | { readonly valueType: "string"; readonly values: readonly string[] };

/** One entry: its leaves and the cumulative offsets of each array level, outermost first. */
export interface Column {
  readonly name: string;
  readonly leaves: Leaves;
  readonly levels: readonly Uint32Array[];
}

export function decode(bytes: Uint8Array): Column[] {
  if (!littleEndian) {
    throw new Error("gecode-iris requires a little-endian platform");
  }
  // Views need 8-byte alignment from the start of the ArrayBuffer.
  const buffer = bytes.byteOffset % 8 === 0 ? bytes : bytes.slice();
  const reader = new Reader(buffer);
  const version = reader.u32();
  if (version !== FORMAT_VERSION) {
    throw new Error(`unknown transfer format ${version}`);
  }
  const columns: Column[] = [];
  for (let count = reader.u32(); count > 0; count--) {
    const name = reader.string();
    const valueType = VALUE_TYPES[reader.u32()];
    if (valueType === undefined) {
      throw new Error("unknown value type in transfer buffer");
    }
    const levels: Uint32Array[] = [];
    for (let depth = reader.u32(); depth > 0; depth--) {
      levels.push(reader.u32Array(reader.u32()));
    }
    columns.push({ name, leaves: reader.leaves(valueType), levels });
  }
  return columns;
}

class Reader {
  readonly #bytes: Uint8Array;
  readonly #view: DataView;
  #position = 0;

  constructor(bytes: Uint8Array) {
    this.#bytes = bytes;
    this.#view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  u32(): number {
    const value = this.#view.getUint32(this.#position, true);
    this.#position += 4;
    return value;
  }

  string(): string {
    const length = this.u32();
    const text = utf8.decode(this.#take(length));
    this.#align(4);
    return text;
  }

  u32Array(count: number): Uint32Array {
    this.#align(8);
    return new Uint32Array(this.#bytes.buffer, this.#offset(count * 4), count);
  }

  leaves(valueType: ValueType): Leaves {
    const count = this.u32();
    if (valueType === "string") {
      const offsets = this.u32Array(count + 1);
      const blob = this.#take(offsets[count] ?? 0);
      this.#align(4);
      const values = Array.from({ length: count }, (_, index) =>
        utf8.decode(blob.subarray(offsets[index], offsets[index + 1])),
      );
      return { valueType, values };
    }
    this.#align(8);
    const offset = this.#offset(count * 8);
    return valueType === "int"
      ? { valueType, values: new BigInt64Array(this.#bytes.buffer, offset, count) }
      : { valueType, values: new Float64Array(this.#bytes.buffer, offset, count) };
  }

  /** Advances past `length` bytes and returns their absolute offset in the ArrayBuffer. */
  #offset(length: number): number {
    if (this.#position + length > this.#bytes.byteLength) {
      throw new RangeError("transfer buffer is truncated");
    }
    const offset = this.#bytes.byteOffset + this.#position;
    this.#position += length;
    return offset;
  }

  #take(length: number): Uint8Array {
    const start = this.#position;
    this.#offset(length);
    return this.#bytes.subarray(start, start + length);
  }

  #align(alignment: number): void {
    this.#position = Math.ceil(this.#position / alignment) * alignment;
  }
}
