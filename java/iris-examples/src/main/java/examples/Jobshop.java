package examples;

import dev.gecode.iris.Data;
import dev.gecode.iris.Params;
import dev.gecode.iris.processor.IrisSpec;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

/** The job shop reader spec, whose handles and record iris-processor writes when this compiles. */
@IrisSpec("formats/jobshop/jobshop.iris")
public final class Jobshop {
  private Jobshop() {}

  public static void main(String[] args) {
    Path orLibrary = Path.of("formats/jobshop/cases/or-library/input.txt");
    Data data = JobshopHandles.spec.read(orLibrary, Params.of("instance", "handmade-or"));
    List<long[]> machines = data.get(JobshopHandles.machine);
    System.out.printf(
        "%s: %d jobs, the first on machines %s%n",
        data.get(JobshopHandles.name),
        data.get(JobshopHandles.nJobs),
        Arrays.toString(machines.get(0)));

    Path jsplib = Path.of("formats/jobshop/cases/jsplib/input.txt");
    JobshopData shop = JobshopHandles.reader.read(jsplib);
    System.out.printf(
        "%d jobs on %d machines, the first with durations %s%n",
        shop.nJobs(), shop.nMachines(), Arrays.toString(shop.duration().get(0)));
  }
}
