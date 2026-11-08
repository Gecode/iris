import { readFileSync } from "node:fs";
import { basename } from "node:path";
import {
  atLeast,
  distinct,
  div,
  gecode,
  IrisError,
  p,
  range,
  spec,
  sum,
  word,
} from "gecode-iris/build";

/**
 * Reads OR-Library and JSPLIB files: a header line with the numbers of jobs
 * and machines, then one line per job of machine and duration pairs.
 */
export function jobshop() {
  return spec("jobshop", (b) => {
    b.comment("#", "+++");
    return b.instances((i) => {
      const name = i.optionalCapture("name", word);
      const nJobs = i.capture("n_jobs", atLeast(1));
      const nMachines = i.capture("n_machines", atLeast(1));
      const { job, machine, duration } = i.sections({ skipOther: true }, (s) => {
        s.arm(p`instance ${name}`, { optional: true });
        return s.arm(p`${nJobs} ${nMachines}`, { last: true }, (a) => {
          a.check(
            nJobs.le(div(div(gecode.intMax.minus(2), 2), nMachines)),
            "there are too many jobs and machines",
          );
          return a.repeat(nJobs, (r) => {
            r.asName("jobs"); // error messages name the records
            r.eachName("job");
            return {
              job: r.record,
              ...r.repeat(nMachines, (step) => ({
                machine: step.read("machine", range(0).below(nMachines)),
                duration: step.read("duration", atLeast(0)),
              })),
            };
          });
        });
      });
      i.check(
        distinct(job.get(machine)),
        "a job uses a machine twice; each job must use every machine once",
      );
      i.check(
        sum(job, (j) => sum(j.get(duration))).le(gecode.intMax),
        "the total duration is too large",
      );
      return { name, nJobs, nMachines, machine, duration };
    });
  });
}

function main(): void {
  const shop = jobshop();
  const path = "formats/jobshop/cases/or-library/input.txt";
  const data = shop.read(readFileSync(path), { instance: "handmade-or" }, path);
  console.log(`${data.name ?? "unnamed"}: ${data.nJobs} jobs on ${data.nMachines} machines`);
  for (const [job, machines] of data.machine.entries()) {
    const durations = data.duration[job] ?? [];
    console.log(
      `job ${job}: machines [${machines.join(", ")}], durations [${durations.join(", ")}]`,
    );
  }

  const invalid = "formats/jobshop/cases/duplicate-machine/input.txt";
  try {
    shop.read(readFileSync(invalid), { instance: "handmade-invalid" }, invalid);
  } catch (error) {
    if (!(error instanceof IrisError)) throw error;
    const { inputLocation: at, callSite: site } = error;
    console.log(`\n${error.code} at ${at?.origin}:${at?.line}:${at?.column}`);
    console.log(error.message);
    // The statement's call site: the line of the distinct check in jobshop().
    console.log(`call site: ${basename(site?.file ?? "")}:${site?.line}`);
  }
}

if (import.meta.main) {
  main();
}
