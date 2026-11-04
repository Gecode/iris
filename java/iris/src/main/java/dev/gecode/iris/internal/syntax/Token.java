package dev.gecode.iris.internal.syntax;

/**
 * A token of a reader spec.
 *
 * @param kind the kind of token
 * @param text the spelling, or the decoded characters of a string
 * @param integer the value of an integer
 * @param real the value of a float
 * @param line the line in the reader spec
 * @param glued whether no white space or comment comes before the token
 */
public record Token(Kind kind, String text, long integer, double real, int line, boolean glued) {
  /** The kinds of token. */
  public enum Kind {
    IDENTIFIER,
    INTEGER,
    FLOAT,
    STRING,
    PUNCTUATION,
    END
  }

  static Token of(Kind kind, String text, int line, boolean glued) {
    return new Token(kind, text, 0, 0.0, line, glued);
  }
}
