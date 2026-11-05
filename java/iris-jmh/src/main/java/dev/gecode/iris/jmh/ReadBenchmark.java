package dev.gecode.iris.jmh;

import dev.gecode.iris.Data;
import dev.gecode.iris.Params;
import dev.gecode.iris.Spec;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.TimeUnit;
import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.annotations.BenchmarkMode;
import org.openjdk.jmh.annotations.Mode;
import org.openjdk.jmh.annotations.OutputTimeUnit;
import org.openjdk.jmh.annotations.Param;
import org.openjdk.jmh.annotations.Scope;
import org.openjdk.jmh.annotations.Setup;
import org.openjdk.jmh.annotations.State;

/**
 * Measures {@link Spec#read(byte[], String, Params)} on each {@link FormatInput}.
 *
 * <p>The input is in memory and the spec is already parsed, so neither parsing nor file access is
 * part of the measurement.
 */
@State(Scope.Benchmark)
@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.MICROSECONDS)
public class ReadBenchmark {
  private static final String ORIGIN = "input.txt";

  /** The input to read, which JMH sets to each constant in turn. */
  @Param public FormatInput input = FormatInput.DIMACS_THREE_VARIABLES;

  private Spec spec;
  private byte[] bytes;
  private Params params;

  /** Creates the benchmark, as JMH requires. */
  public ReadBenchmark() {}

  /**
   * Parses the spec and loads the input, outside the measurement.
   *
   * @throws IOException if a file of the format library cannot be read
   */
  @Setup
  public void load() throws IOException {
    Path formats = Path.of(System.getProperty("iris.formats"));
    spec = Spec.parse(Files.readAllBytes(input.spec(formats)), input.spec(formats).toString());
    bytes = Files.readAllBytes(input.file(formats));
    params = input.params();
  }

  /**
   * Reads the input.
   *
   * @return the data, which JMH consumes
   */
  @Benchmark
  public Data read() {
    return spec.read(bytes, ORIGIN, params);
  }
}
