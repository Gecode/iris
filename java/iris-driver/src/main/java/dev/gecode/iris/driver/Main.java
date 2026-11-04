package dev.gecode.iris.driver;

/** The entry point of the conformance driver. */
public final class Main {
  private Main() {}

  /**
   * Runs one driver request and exits with its status.
   *
   * @param args the driver command and its arguments
   */
  public static void main(String[] args) {
    System.exit(Driver.run(args, System.out, System.err));
  }
}
