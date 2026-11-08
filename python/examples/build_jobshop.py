from __future__ import annotations

from pathlib import Path
from typing import NamedTuple

import gecode_iris as iris
import gecode_iris.build as ib


class Jobshop(NamedTuple):
    """A job shop reader and the entries a scheduler needs."""

    spec: iris.Spec
    name: ib.StrField[str | None]
    n_jobs: ib.IntField[int]
    n_machines: ib.IntField[int]
    machine: ib.IntField[list[list[int]]]
    duration: ib.IntField[list[list[int]]]


def jobshop() -> Jobshop:
    """Reads OR-Library and JSPLIB files: a header line with the numbers of
    jobs and machines, then one line per job of machine and duration pairs."""
    b = ib.SpecBuilder("jobshop")
    b.comment("#", "+++")
    with b.instances() as i:
        name = i.optional_capture("name", ib.word)
        n_jobs = i.capture("n_jobs", ib.at_least(1))
        n_machines = i.capture("n_machines", ib.at_least(1))
        with i.sections(skip_other=True) as arms:
            arms.arm(("instance ", name), optional=True)
            with arms.arm((n_jobs, " ", n_machines), last=True) as a:
                a.check(
                    n_jobs <= ib.div(ib.div(ib.gecode.INT_MAX - 2, 2), n_machines),
                    "there are too many jobs and machines",
                )
                with a.repeat(n_jobs) as job:
                    job.as_name("jobs")  # error messages name the records
                    with job.repeat(n_machines) as step:
                        machine = step.read("machine", ib.int_range(0, below=n_machines))
                        duration = step.read("duration", ib.at_least(0))
                    jobs = job.record
        i.check(
            ib.distinct(jobs[machine]),
            "a job uses a machine twice; each job must use every machine once",
        )
        i.check(
            ib.sum(jobs, lambda j: ib.sum(j[duration])) <= ib.gecode.INT_MAX,
            "the total duration is too large",
        )
    return Jobshop(b.build(), name, n_jobs, n_machines, machine, duration)


def main() -> None:
    shop = jobshop()
    data = shop.spec.read(
        "formats/jobshop/cases/or-library/input.txt", params={"instance": "handmade-or"}
    )
    name = data[shop.name] or "unnamed"
    print(f"{name}: {data[shop.n_jobs]} jobs on {data[shop.n_machines]} machines")
    steps = zip(data[shop.machine], data[shop.duration], strict=True)
    for job, (machines, durations) in enumerate(steps):
        print(f"job {job}: machines {machines}, durations {durations}")

    try:
        shop.spec.read(
            "formats/jobshop/cases/duplicate-machine/input.txt",
            params={"instance": "handmade-invalid"},
        )
    except iris.Error as error:
        print(f"\n{error.code} at {error.origin}:{error.line}:{error.column}")
        print(error.message)
        # The statement's call site: the line of the i.check call above.
        site = error.call_site
        assert site is not None
        print(f"call site: {Path(site.filename).name}:{site.line}, in {site.function}()")


if __name__ == "__main__":
    main()
