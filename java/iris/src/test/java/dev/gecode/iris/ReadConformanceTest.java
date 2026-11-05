package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.ExecutionException;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.DisabledOnOs;
import org.junit.jupiter.api.condition.OS;
import org.junit.jupiter.api.io.TempDir;

class ReadConformanceTest {
  @Test
  void everyReadCaseOffTheDeferredListPasses()
      throws IOException, InterruptedException, ExecutionException {
    ConformanceSuite.fromBuild().requireOutcomes("read", "/deferred-cases.txt");
  }

  /** The shell script holds the argument's bytes, so this JVM's locale cannot change them. */
  @Test
  @DisabledOnOs(OS.WINDOWS)
  void theLauncherKeepsParamsOrRefusesThem(@TempDir Path directory)
      throws IOException, InterruptedException {
    Path spec = Files.writeString(directory.resolve("spec.iris"), "param p = \"\"\n");
    Path input = Files.createFile(directory.resolve("input.txt"));
    Path script =
        Files.writeString(
            directory.resolve("run.sh"),
            "exec \"$1\" read \"$2\" \"$3\" --params '{\"p\":\"é😀\"}'\n",
            UTF_8);
    var launcher =
        new ProcessBuilder(
            "/bin/sh",
            script.toString(),
            System.getProperty("iris.driver"),
            spec.toString(),
            input.toString());
    launcher.environment().put("LC_ALL", "C");
    launcher.environment().put("JAVA_HOME", System.getProperty("java.home"));
    Process process = launcher.redirectError(ProcessBuilder.Redirect.DISCARD).start();
    String output = new String(process.getInputStream().readAllBytes(), UTF_8);
    int status = process.waitFor();
    assertTrue(status == 0 ? output.contains("é😀") : status == 3 && output.isEmpty(), output);
  }
}
