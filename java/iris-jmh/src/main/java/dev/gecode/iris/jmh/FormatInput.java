package dev.gecode.iris.jmh;

import dev.gecode.iris.Params;
import java.nio.file.Path;
import org.jspecify.annotations.Nullable;

/** The inputs of the format library that read without an error. */
public enum FormatInput {
  /** A DIMACS CNF formula. */
  DIMACS_THREE_VARIABLES("dimacs-cnf", "three-variables"),
  /** A job shop instance in the JSPLIB layout. */
  JOBSHOP_JSPLIB("jobshop", "jsplib"),
  /** One named instance of a job shop file in the OR-Library layout. */
  JOBSHOP_OR_LIBRARY("jobshop", "or-library", "handmade-or"),
  /** A nonogram. */
  NONOGRAM_FIVE_BY_FIVE("nonogram", "five-by-five"),
  /** A sudoku. */
  SUDOKU_FOUR_BY_FOUR("sudoku", "four-by-four"),
  /** A TSPLIB instance with coordinates. */
  TSPLIB_EUC_2D_FIVE("tsplib", "euc-2d-five"),
  /** A TSPLIB instance with an explicit matrix. */
  TSPLIB_EXPLICIT_FIVE("tsplib", "explicit-five"),
  /** A travelling salesman instance with time windows. */
  TSPTW_THREE_NODES("tsptw", "three-nodes");

  private final String format;
  private final String caseName;
  private final @Nullable String instance;

  FormatInput(String format, String caseName) {
    this(format, caseName, null);
  }

  FormatInput(String format, String caseName, @Nullable String instance) {
    this.format = format;
    this.caseName = caseName;
    this.instance = instance;
  }

  /** Returns the file of the reader spec under the format library {@code formats}. */
  Path spec(Path formats) {
    return formats.resolve(format).resolve(format + ".iris");
  }

  /** Returns the input file under the format library {@code formats}. */
  Path file(Path formats) {
    return formats.resolve(format).resolve("cases").resolve(caseName).resolve("input.txt");
  }

  /** Returns the parameters that the input's case reads it with. */
  Params params() {
    return instance == null ? Params.of() : Params.of("instance", instance);
  }
}
