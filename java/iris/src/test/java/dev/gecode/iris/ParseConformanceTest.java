package dev.gecode.iris;

import java.io.IOException;
import org.junit.jupiter.api.Test;

class ParseConformanceTest {
  @Test
  void everyParseCasePasses() throws IOException, InterruptedException {
    ConformanceSuite.fromBuild().requireAllPass("parse");
  }
}
