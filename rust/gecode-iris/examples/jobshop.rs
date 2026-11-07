use gecode_iris::build::{
    BuildError, D0, D2, Field, Int, Maybe, SpecBuilder, Str, distinct, gecode, lit, pattern, sum,
    sum_over, word,
};
use gecode_iris::{Params, Spec};

/// A job shop reader and the entries a scheduler needs.
pub struct Jobshop {
    pub spec: Spec,
    pub name: Field<Str, D0, Maybe>,
    pub n_jobs: Field<Int>,
    pub n_machines: Field<Int>,
    pub machine: Field<Int, D2>,
    pub duration: Field<Int, D2>,
}

/// Reads OR-Library and JSPLIB files: a header line with the numbers of jobs
/// and machines, then one line per job of machine and duration pairs.
pub fn jobshop() -> Result<Jobshop, BuildError> {
    let mut b = SpecBuilder::new("jobshop");
    b.comment(["#", "+++"]);
    let (name, n_jobs, n_machines, machine, duration) = b.instances(|i| {
        let name = i.optional_capture("name", word());
        let n_jobs = i.capture("n_jobs", 1..);
        let n_machines = i.capture("n_machines", 1..);
        let (job, machine, duration) = i.sections().skip_other().body(|s| {
            s.arm(pattern!("instance {name}")).optional();
            s.arm(pattern!("{n_jobs} {n_machines}")).last().body(|a| {
                a.check(n_jobs.le((gecode::INT_MAX - 2) / 2 / n_machines))
                    .message("there are too many jobs and machines");
                a.repeat(n_jobs, |job| {
                    job.as_name("jobs"); // error messages name the records
                    let (machine, duration) = job.repeat(n_machines, |step| {
                        let machine = step.read("machine", lit(0)..n_machines.ex());
                        let duration = step.read("duration", 0..);
                        (machine, duration)
                    });
                    (job.record(), machine, duration)
                })
            })
        });
        i.check(distinct(job.get(machine)))
            .message("a job uses a machine twice; each job must use every machine once");
        i.check(sum_over(job, |j| sum(j.get(duration))).le(gecode::INT_MAX))
            .message("the total duration is too large");
        (name, n_jobs, n_machines, machine, duration)
    });
    Ok(Jobshop {
        spec: b.build()?,
        name,
        n_jobs,
        n_machines,
        machine,
        duration,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jobshop = jobshop()?;

    let mut params = Params::new();
    params.set("instance", "handmade-or");
    let data = jobshop
        .spec
        .read_path("formats/jobshop/cases/or-library/input.txt", &params)?;
    let name: Option<&str> = data.get(jobshop.name);
    let jobs: i64 = data.get(jobshop.n_jobs);
    let machines: i64 = data.get(jobshop.n_machines);
    println!(
        "{}: {jobs} jobs on {machines} machines",
        name.unwrap_or("unnamed")
    );
    let steps = data.get(jobshop.machine).rows();
    for (job, (machine, duration)) in steps.zip(data.get(jobshop.duration).rows()).enumerate() {
        println!("job {job}: machines {machine:?}, durations {duration:?}");
    }

    params.set("instance", "handmade-invalid");
    let path = "formats/jobshop/cases/duplicate-machine/input.txt";
    if let Err(error) = jobshop.spec.read_path(path, &params) {
        println!("\n{error}");
    }
    Ok(())
}
