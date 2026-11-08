package examples;

import static dev.gecode.iris.build.Iris.*;

import dev.gecode.iris.Data;
import dev.gecode.iris.Entry;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Params;
import dev.gecode.iris.RecordReader;
import dev.gecode.iris.Spec;
import dev.gecode.iris.build.Gecode;
import dev.gecode.iris.build.IntField;
import dev.gecode.iris.build.RecordName;
import dev.gecode.iris.build.StringField;
import java.io.IOException;
import java.lang.invoke.MethodHandles;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import org.jspecify.annotations.Nullable;

/** A job shop reader and the entries a scheduler needs. */
public final class BuildJobshop {
  public static final StringField<@Nullable String> name = string0("name").optional();
  public static final IntField<Long> nJobs = int0("n_jobs");
  public static final IntField<Long> nMachines = int0("n_machines");
  public static final IntField<List<long[]>> machine = int2("machine");
  public static final IntField<List<long[]>> duration = int2("duration");

  // Error messages name the records of the jobs.
  private static final RecordName job = recordName("job", "jobs");

  /**
   * Reads OR-Library and JSPLIB files: a header line with the numbers of jobs and machines, then
   * one line per job of machine and duration pairs.
   */
  public static final Spec spec =
      spec(
          "jobshop",
          () -> {
            comment("#", "+++");
            instances(() -> {
              sections(
                  skipOther(),
                  () -> {
                    arm(pattern("instance ", capture(name, word())), optional());
                    var header =
                        pattern(capture(nJobs, atLeast(1)), " ", capture(nMachines, atLeast(1)));
                    arm(
                        header,
                        last(),
                        () -> {
                          check(
                              nJobs.le(Gecode.intMax.minus(2).div(2).div(nMachines)),
                              "there are too many jobs and machines");
                          repeat(
                              nJobs,
                              job,
                              () ->
                                  repeat(
                                      nMachines,
                                      () -> {
                                        read(machine, range(0).below(nMachines));
                                        read(duration, atLeast(0));
                                      }));
                        });
                  });
              check(
                  distinct(job.get(machine)),
                  "a job uses a machine twice; each job must use every machine once");
              check(
                  sum(job, j -> sum(j.get(duration))).le(Gecode.intMax),
                  "the total duration is too large");
            });
          });

  /** The same entries as a record, whose components name them. */
  record Shop(
      @Nullable String name,
      @Entry("n_jobs") long nJobs,
      @Entry("n_machines") long nMachines,
      List<long[]> machine,
      List<long[]> duration) {}

  private BuildJobshop() {}

  public static void main(String[] args) throws IOException {
    Path orLibrary = Path.of("formats/jobshop/cases/or-library/input.txt");
    Data data = spec.read(orLibrary, Params.of("instance", "handmade-or"));
    String shop = data.get(name);
    System.out.printf(
        "%s: %d jobs on %d machines%n",
        shop == null ? "unnamed" : shop, data.get(nJobs), data.get(nMachines));
    List<long[]> machines = data.get(machine);
    List<long[]> durations = data.get(duration);
    for (int j = 0; j < machines.size(); j++) {
      System.out.printf(
          "job %d: machines %s, durations %s%n",
          j, Arrays.toString(machines.get(j)), Arrays.toString(durations.get(j)));
    }

    RecordReader<Shop> reader = spec.reader(Shop.class, MethodHandles.lookup());
    Shop jsplib = reader.read(Path.of("formats/jobshop/cases/jsplib/input.txt"));
    System.out.printf(
        "%n%s: %d jobs, first job on machines %s%n",
        jsplib.name() == null ? "unnamed" : jsplib.name(),
        jsplib.nJobs(),
        Arrays.toString(jsplib.machine().get(0)));

    // Errors name a path's input with the platform's separators, so this read
    // names the input itself to print the same on every platform.
    String invalid = "formats/jobshop/cases/duplicate-machine/input.txt";
    try {
      var _ =
          spec.read(
              Files.readAllBytes(Path.of(invalid)),
              invalid,
              Params.of("instance", "handmade-invalid"));
    } catch (IrisException error) {
      System.out.printf(
          "%n%s at %s:%d:%d%n", error.code().code(), error.origin(), error.line(), error.column());
      System.out.println(error.error().message());
      // The statement's call site: the line of the distinct check above.
      StackTraceElement site = error.callSite().orElseThrow();
      System.out.println("call site: " + site.getFileName() + ":" + site.getLineNumber());
    }
  }
}
