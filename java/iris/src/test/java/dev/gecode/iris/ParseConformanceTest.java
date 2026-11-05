package dev.gecode.iris;

import java.io.IOException;
import java.util.concurrent.ExecutionException;
import org.junit.jupiter.api.Test;

class ParseConformanceTest {
  @Test
  void everyParseCasePasses() throws IOException, InterruptedException, ExecutionException {
    ConformanceSuite.fromBuild().requireAllPass("parse");
  }
}
