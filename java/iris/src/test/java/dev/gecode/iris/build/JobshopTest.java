package dev.gecode.iris.build;

import static dev.gecode.iris.build.Iris.*;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.regex.Matcher;
import java.util.stream.Stream;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

/** The running example of {@code docs/builders.md}. */
class JobshopTest {
  static final String MSG_DISTINCT =
      "a job uses a machine twice; each job must use every machine once";
  static final String MSG_TOTAL = "the total duration is too large";

  static final StringField<@Nullable String> name = string0("name").optional();
  static final IntField<Long> nJobs = int0("n_jobs");
  static final IntField<Long> nMachines = int0("n_machines");
  static final IntField<List<long[]>> machine = int2("machine");
  static final IntField<List<long[]>> duration = int2("duration");
  static final RecordName job = recordName();

  static final Builder builder =
      builder(
          "jobshop",
          () -> {
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
                              () ->
                                  repeat(
                                      nMachines,
                                      () -> {
                                        read(machine, range(0).below(nMachines));
                                        read(duration, atLeast(0));
                                      }));
                        });
                  });
              check(distinct(job.get(machine)), MSG_DISTINCT);
              check(sum(job, j -> sum(j.get(duration))).le(Gecode.intMax), MSG_TOTAL);
            });
          });

  @Test
  void theBuilderPrintsTheRunningExample() {
    assertEquals(
        """
        format jobshop
        comment "#" "+++"
        instances {
          sections skip other {
            "instance {name:word}" optional
            "{n_jobs:1..} {n_machines:1..}" last {
              check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"
              repeat n_jobs as records_1 {
                each record_1
                repeat n_machines {
                  machine: 0..<n_machines
                  duration: 0..
                }
              }
            }
          }
          check distinct(record_1.machine) "a job uses a machine twice; each job must use every machine once"
          check sum(element_1 in records_1: sum(element_1.duration)) <= gecode.int_max "the total duration is too large"
        }
        """,
        builder.source());
  }

  /** For every case of {@code formats/jobshop}, the built spec reads what the text spec reads. */
  @Test
  void theBuiltSpecReadsWhatTheTextSpecReads() throws IOException {
    Path format = repository().resolve("formats/jobshop");
    Spec text = Spec.parse(Files.readString(format.resolve("jobshop.iris")));
    Spec built = builder.build();
    int read = 0;
    int failed = 0;
    try (Stream<Path> cases = Files.list(format.resolve("cases"))) {
      for (Path dir : cases.sorted().toList()) {
        String input = Files.readString(dir.resolve("input.txt"));
        Path paramsFile = dir.resolve("params.json");
        Params params =
            Files.exists(paramsFile)
                ? Params.of("instance", instance(Files.readString(paramsFile)))
                : Params.of();
        Data expected;
        try {
          expected = text.read(input, "input", params);
        } catch (IrisException error) {
          var again = assertThrows(IrisException.class, () -> built.read(input, "input", params));
          assertEquals(error.code(), again.code());
          failed++;
          continue;
        }
        Data data = built.read(input, "input", params);
        assertEquals(expected.toJson(), data.toJson(), dir.toString());
        assertEquals(
            expected.find("name") == null ? null : expected.get("name").asString(), data.get(name));
        assertEquals(expected.get("n_jobs").asLong(), data.get(nJobs));
        assertEquals(expected.get("n_machines").asLong(), data.get(nMachines));
        List<long[]> rows = data.get(machine);
        assertEquals(expected.get("machine").size(), rows.size());
        for (int k = 0; k < rows.size(); k++) {
          assertArrayEquals(expected.get("machine").get(k).longs(), rows.get(k));
          assertArrayEquals(expected.get("duration").get(k).longs(), data.get(duration).get(k));
        }
        read++;
      }
    }
    assertEquals(2, read);
    assertEquals(1, failed);
  }

  private static String instance(String json) {
    Matcher match = java.util.regex.Pattern.compile("\"instance\":\\s*\"([^\"]*)\"").matcher(json);
    assertTrue(match.find(), json);
    return match.group(1);
  }

  private static Path repository() {
    return Path.of(System.getProperty("iris.repository"));
  }
}
