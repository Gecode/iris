// Conformance driver that reads with a format's hand-written builder.
//
// The first argument names a format directory under formats/, such as
// dimacs-cnf; the rest is the protocol of test/conformance-driver.ts. The
// driver builds the spec with examples/build-dimacs-cnf.ts, through the
// function it exports under the format's name in lower camel case,
// dimacsCnf, and answers with that spec in place of the case's own, so the
// runner judges each case of the format against what the builder builds:
//
//   node test/format-builder-driver.ts jobshop read SPEC INPUT
import { Spec } from "gecode-iris";
import { EXIT_MISUSE, main, report } from "./conformance-driver.ts";

/** The spec that the builder of `format` builds. */
async function builtSpec(format: string): Promise<Spec> {
  const module: unknown = await import(
    new URL(`../examples/build-${format}.ts`, import.meta.url).href
  );
  const name = format.replaceAll(/-(\w)/g, (_, letter: string) => letter.toUpperCase());
  const builder: unknown = Reflect.get(Object(module), name);
  if (typeof builder !== "function") {
    throw new TypeError(`examples/build-${format}.ts exports no function ${name}`);
  }
  const spec: unknown = Reflect.get(Object(Reflect.apply(builder, undefined, [])), "spec");
  if (!(spec instanceof Spec)) {
    throw new TypeError(`${name}() returns no built spec`);
  }
  return spec;
}

const [format, ...request] = process.argv.slice(2);
if (format === undefined) {
  report(EXIT_MISUSE, "expected a format name");
} else {
  const spec = await builtSpec(format);
  main(request, () => spec);
}
