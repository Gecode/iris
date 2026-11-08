import { readFileSync } from "node:fs";
import { atLeast, gecode, p, range, spec } from "gecode-iris/build";

/** Reads the number of variables and one row of literals per clause. */
export function dimacsCnf() {
  return spec("dimacs-cnf", (b) => {
    b.comment("c");
    b.stop("%"); // SATLIB files end with "%" and "0"
    const vars = b.capture("vars", range(0, gecode.intMax));
    const clauses = b.capture("clauses", atLeast(0));
    b.line(p`p cnf ${vars} ${clauses}`);
    // The terminating 0 is not stored.
    const clause = b.repeat(clauses, (c) => c.list("clause", range(vars.neg(), vars).until(0)));
    return { vars, clause };
  });
}

if (import.meta.main) {
  const data = dimacsCnf().read(readFileSync("formats/dimacs-cnf/cases/three-variables/input.txt"));
  console.log(`${data.vars} variables`);
  for (const clause of data.clause) {
    console.log(clause.join(" "));
  }
}
