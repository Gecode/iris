use gecode_iris::build::*;

pub fn builder() -> SpecBuilder {
    let mut b = SpecBuilder::new("jobshop");
    let name: DynField = b.forward("name");
    let n_jobs: DynField = b.forward("n_jobs");
    let n_machines: DynField = b.forward("n_machines");
    let machine: DynField = b.forward("machine");
    let duration: DynField = b.forward("duration");
    let job = b.forward_record("job", "jobs");
    b.comment(["#", "+++"]);
    b.instances(|s| {
        s.sections().skip_other().body(|a| {
            a.arm(Pattern::new().text("instance ").part(bind(name, word())))
                .optional();
            let e1 = Pattern::new()
                .part(bind(n_jobs, 1..))
                .text(" ")
                .part(bind(n_machines, 1..));
            a.arm(e1).last().body(|s| {
                s.check(n_jobs.le((gecode::INT_MAX - 2) / 2 / n_machines))
                    .message("there are too many jobs and machines");
                s.repeat(n_jobs, |s| {
                    s.record_as(job);
                    s.repeat(n_machines, |s| {
                        s.each_name("step");
                        s.as_name("steps");
                        s.read(machine, lit(0).dynamic()..n_machines.dynamic());
                        s.read(duration, 0..);
                    });
                });
            });
        });
        s.check(distinct(job.get(machine)))
            .message("a job uses a machine twice; each job must use every machine once");
        let e2 = sum_over(over(job.list()).named("j"), |j| {
            sum(j.clone().get(duration))
        });
        s.check(e2.le(gecode::INT_MAX))
            .message("the total duration is too large");
    });
    b
}

fn main() {
    let source = builder().source().expect("the spec is valid");
    print!("{source}");
}
