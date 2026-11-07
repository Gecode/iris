import gecode_iris.build as ib


def builder() -> ib.SpecBuilder:
    b = ib.SpecBuilder("jobshop")
    name = b.forward("name", ib.DynField)
    n_jobs = b.forward("n_jobs", ib.DynField)
    n_machines = b.forward("n_machines", ib.DynField)
    machine = b.forward("machine", ib.DynField)
    duration = b.forward("duration", ib.DynField)
    job = b.forward_record("job", "jobs")
    b.comment("#", "+++")
    with b.instances() as s1:
        with s1.sections(skip_other=True) as a2:
            a2.arm(("instance ", ib.bind(name, ib.word)), optional=True)
            e1 = (
                ib.bind(n_jobs, ib.at_least(1)),
                " ",
                ib.bind(n_machines, ib.at_least(1)),
            )
            with a2.arm(e1, last=True) as s3:
                s3.check(
                    n_jobs <= ib.div(ib.div(ib.gecode.INT_MAX - 2, 2), n_machines),
                    "there are too many jobs and machines",
                )
                with s3.repeat(n_jobs) as s4:
                    s4.record_as(job)
                    with s4.repeat(n_machines) as s5:
                        s5.each_name("step")
                        s5.as_name("steps")
                        s5.read(machine, ib.int_range(0, below=n_machines))
                        s5.read(duration, ib.at_least(0))
        s1.check(
            ib.distinct(job[machine]),
            "a job uses a machine twice; each job must use every machine once",
        )
        e2 = ib.sum(ib.over(job.list, "j"), lambda j: ib.sum(j[duration]))
        s1.check(e2 <= ib.gecode.INT_MAX, "the total duration is too large")
    return b


if __name__ == "__main__":
    print(builder().source(), end="")
