import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.build.Builder;
import dev.gecode.iris.build.DynField;
import dev.gecode.iris.build.Gecode;
import dev.gecode.iris.build.RecordName;

final class SpecBuilder {
  private static final DynField name = dynamic("name");
  private static final DynField nJobs = dynamic("n_jobs");
  private static final DynField nMachines = dynamic("n_machines");
  private static final DynField machine = dynamic("machine");
  private static final DynField duration = dynamic("duration");
  private static final RecordName job = recordName("job", "jobs");

  private SpecBuilder() {}

  static Builder specBuilder() {
    return builder("jobshop", SpecBuilder::body);
  }

  private static void body() {
    comment("#", "+++");
    instances(() -> {
      sections(
          skipOther(),
          () -> {
            arm(pattern("instance ", capture(name, word())), optional());
            arm(
                pattern(capture(nJobs, atLeast(1)), " ", capture(nMachines, atLeast(1))),
                last(),
                () -> {
                  check(
                      nJobs.le(Gecode.intMax.minus(2).div(2).div(nMachines)),
                      "there are too many jobs and machines");
                  repeat(
                      nJobs,
                      job,
                      () -> {
                        repeat(
                            nMachines,
                            recordName("step", "steps"),
                            () -> {
                              read(machine, range(0).below(nMachines));
                              read(duration, atLeast(0));
                            });
                      });
                });
          });
      check(
          distinct(job.get(machine)),
          "a job uses a machine twice; each job must use every machine once");
      check(
          sum(over(job.list()).named("j"), j -> sum(j.get(duration))).le(Gecode.intMax),
          "the total duration is too large");
    });
  }

  public static void main(String[] args) {
    System.out.print(specBuilder().source());
  }
}
