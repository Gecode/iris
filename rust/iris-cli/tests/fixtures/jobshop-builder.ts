import {
  SpecBuilder,
  atLeast,
  bind,
  distinct,
  div,
  gecode,
  over,
  p,
  range,
  sum,
  word,
} from "gecode-iris/build";

export function builder(): SpecBuilder {
  const b = new SpecBuilder("jobshop");
  const name = b.forward("name");
  const n_jobs = b.forward("n_jobs");
  const n_machines = b.forward("n_machines");
  const machine = b.forward("machine");
  const duration = b.forward("duration");
  const job = b.forwardRecord("job", "jobs");
  b.comment("#", "+++");
  b.instances((s1) => {
    s1.sections({ skipOther: true }, (a2) => {
      a2.arm(p`instance ${bind(name, word)}`, { optional: true });
      const e1 = p`${bind(n_jobs, atLeast(1))} ${bind(n_machines, atLeast(1))}`;
      a2.arm(e1, { last: true }, (s3) => {
        s3.check(
          n_jobs.le(div(div(gecode.intMax.minus(2), 2), n_machines)),
          "there are too many jobs and machines",
        );
        s3.repeat(n_jobs, (s4) => {
          s4.recordAs(job);
          s4.repeat(n_machines, (s5) => {
            s5.eachName("step");
            s5.asName("steps");
            s5.read(machine, range(0).below(n_machines));
            s5.read(duration, atLeast(0));
          });
        });
      });
    });
    s1.check(
      distinct(job.get(machine)),
      "a job uses a machine twice; each job must use every machine once",
    );
    const e2 = sum(over(job.list, "j"), (j) => sum(j.get(duration)));
    s1.check(e2.le(gecode.intMax), "the total duration is too large");
  });
  return b;
}

if (import.meta.main) {
  process.stdout.write(builder().source());
}
