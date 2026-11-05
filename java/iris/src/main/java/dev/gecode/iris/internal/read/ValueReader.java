package dev.gecode.iris.internal.read;

import static dev.gecode.iris.internal.read.Execution.quote;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.ReadingKind;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import dev.gecode.iris.internal.syntax.Numbers;
import dev.gecode.iris.internal.syntax.Utf8;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.IdentityHashMap;
import java.util.OptionalDouble;
import java.util.OptionalLong;
import org.jspecify.annotations.Nullable;

/** Reads values from the tokens of the input: scalars, arrays, lists and alphabets (§6). */
final class ValueReader {
  /** What a byte is for an alphabet, unless it is the index of a symbol. */
  private static final int STRIPPED = -1;

  private static final int UNKNOWN = -2;
  private static final int BLANK = -3;

  private final Execution execution;
  private final Input input;
  private final Evaluator evaluator;

  /** For each alphabet used, what each byte is. */
  private final IdentityHashMap<ReadingType.Alphabet, int[]> symbolTables = new IdentityHashMap<>();

  /** Where the last token read as a value starts. */
  private int tokenStart = 0;

  ValueReader(Execution execution, Evaluator evaluator) {
    this.execution = execution;
    this.input = execution.input();
    this.evaluator = evaluator;
  }

  /** Tests whether a value of {@code kind} is a string. */
  static boolean readsStrings(ReadingKind kind) {
    return kind == ReadingKind.WORD || kind == ReadingKind.REST;
  }

  /** Returns the type of the empty list of {@code kind}. */
  private static ValueType listType(ReadingKind kind) {
    return switch (kind) {
      case FLOAT -> ValueType.FLOAT;
      case WORD -> ValueType.STRING;
      case INT, NUM, REST, SYMBOL -> ValueType.INT;
    };
  }

  /**
   * Returns the number of {@code kind}, which does not read strings, in the bytes from {@code from}
   * up to {@code to}, or null if they are not one.
   */
  @Nullable Stored number(ReadingKind kind, int from, int to) {
    if (kind != ReadingKind.FLOAT) {
      OptionalLong integer = Numbers.parseInteger(input.bytes(), from, to);
      if (integer.isPresent()) {
        return new Stored.Int(integer.getAsLong());
      }
    }
    if (kind != ReadingKind.INT) {
      OptionalDouble real = Numbers.parseFloat(input.bytes(), from, to);
      if (real.isPresent()) {
        return new Stored.Real(real.getAsDouble());
      }
    }
    return null;
  }

  /** Fails unless the bytes from {@code from} up to {@code to} of the line are valid (§4.10). */
  void requireValid(int from, int to) {
    int invalid = Utf8.firstInvalid(input.bytes(), from, to);
    if (invalid >= 0) {
      input.moveOnLine(invalid);
      throw execution.failure(ErrorCode.UTF8, "string value is not valid UTF-8");
    }
  }

  /** Returns the failure for want of {@code what} at the end of the input or the token there. */
  private RuntimeException expected(String what) {
    if (!input.skipToData()) {
      return execution.failure(
          ErrorCode.EXPECTED_VALUE, "expected " + what + ", found end of input");
    }
    String found = input.string(input.position(), input.tokenEnd());
    return execution.failure(
        ErrorCode.EXPECTED_VALUE, "expected " + what + ", found " + quote(found));
  }

  /**
   * Reads one value of {@code kind}, or returns null if there is none. At the end of the input or a
   * token that is not of the kind, the position stays before it.
   */
  private @Nullable Stored scan(ReadingKind kind) {
    if (!input.skipToData()) {
      return null;
    }
    tokenStart = input.position();
    if (kind == ReadingKind.REST) {
      int end = input.contentEnd(tokenStart);
      requireValid(tokenStart, end);
      input.toEndOfLine();
      return new Stored.Text(input.string(tokenStart, end));
    }
    int end = input.tokenEnd();
    input.moveOnLine(end);
    if (kind == ReadingKind.WORD) {
      if (end == tokenStart) {
        return null;
      }
      requireValid(tokenStart, end);
      return new Stored.Text(input.string(tokenStart, end));
    }
    Stored value = number(kind, tokenStart, end);
    if (value == null) {
      input.moveOnLine(tokenStart);
    }
    return value;
  }

  /** Reads one integer. */
  long integer() {
    if (scan(ReadingKind.INT) instanceof Stored.Int value) {
      return value.value();
    }
    throw expected("int");
  }

  /** Reads one value of {@code domain}, without checking its range. */
  private Stored scalar(Domain domain) {
    Stored value = scan(domain.kind());
    if (value == null) {
      throw expected(domain.describe());
    }
    return value;
  }

  /** Reads an integer of {@code domain}, which reads integers, without checking its range. */
  private long intScalar(Domain domain) {
    if (!input.skipToData()) {
      throw expected(domain.describe());
    }
    tokenStart = input.position();
    if (!input.readIntToken()) {
      throw expected(domain.describe());
    }
    return input.lastInt();
  }

  /** Returns the failure for the bytes {@code from} up to {@code to} of the current line. */
  RuntimeException outside(Domain domain, int from, int to) {
    String found = input.string(from, to);
    input.moveOnLine(from);
    return execution.failure(
        ErrorCode.EXPECTED_VALUE, "expected " + domain.describe() + ", found " + quote(found));
  }

  /** Reads one value of {@code domain}. */
  Stored value(Domain domain) {
    Stored value = scalar(domain);
    if (!domain.fits(value)) {
      throw outside(domain, tokenStart, input.position());
    }
    return value;
  }

  /** Reads one value of {@code domain} and appends it to {@code leaves}. */
  private void push(Domain domain, Leaves leaves) {
    if (domain.kind() == ReadingKind.INT && leaves.type() == ValueType.INT) {
      long value = intScalar(domain);
      if (!domain.contains(value)) {
        throw outside(domain, tokenStart, input.position());
      }
      leaves.add(value);
    } else {
      leaves.push(value(domain));
    }
  }

  /*
   * Offsets
   */

  /** Returns {@code value} with {@code offset} added, if it is a number (§6.7). */
  Stored withOffset(Stored value, long offset) {
    return switch (value) {
      case Stored.Int number -> {
        try {
          yield new Stored.Int(Math.addExact(number.value(), offset));
        } catch (ArithmeticException _) {
          throw offsetOverflow();
        }
      }
      case Stored.Real number -> new Stored.Real(number.value() + (double) offset);
      case Stored.Text _, Stored.Array _ -> value;
    };
  }

  private void add(Leaves leaves, long offset) {
    if (!leaves.offset(offset)) {
      throw offsetOverflow();
    }
  }

  private RuntimeException offsetOverflow() {
    return execution.failure(ErrorCode.INTEGER_OVERFLOW, "integer overflow when applying offset");
  }

  /*
   * Arrays
   */

  /** Reads an array of {@code domain} with {@code dimensions}, with {@code offset} added. */
  Stored array(Domain domain, int[] dimensions, long offset) {
    Leaves leaves = leavesOf(domain, dimensions);
    add(leaves, offset);
    return Table.shaped(dimensions, leaves);
  }

  /**
   * Reads the values first and makes the structure afterwards, so that a wrong count fails at the
   * end of the input rather than on allocation.
   */
  private Leaves leavesOf(Domain domain, int[] dimensions) {
    // Each factor is at most the largest int, so the product cannot overflow.
    long size = 1;
    for (int dimension : dimensions) {
      size *= dimension;
      if (size > Integer.MAX_VALUE) {
        throw execution.failure(ErrorCode.SIZE_LIMIT, "array is too large");
      }
    }
    // Rows before an empty dimension cost memory without reading any input.
    if (size == 0) {
      long rows = 1;
      for (int k = 0; k < dimensions.length && dimensions[k] > 0; k++) {
        rows *= dimensions[k];
        execution.chargeIdle(rows);
      }
    }
    Leaves leaves = new Leaves(listType(domain.kind()));
    try {
      for (long k = 0; k < size; k++) {
        push(domain, leaves);
      }
    } catch (ShapeError error) {
      throw execution.failure(error, execution.contextText());
    }
    return leaves;
  }

  /**
   * Reads a symmetric {@code n} by {@code n} matrix of {@code domain}, stored in the layout {@code
   * name}, or an {@code n} by {@code m} one for a full matrix (§6.8). The offset applies to the
   * numbers read, not to the cells that only exist after mirroring.
   */
  Stored layout(Domain domain, int n, int m, String name, long offset) {
    if (name.equals("FULL_MATRIX")) {
      return array(domain, new int[] {n, m}, offset);
    }
    if (n != m) {
      throw execution.failure(ErrorCode.LAYOUT, "layout " + name + " needs a square matrix");
    }
    // Column-wise upper equals row-wise lower for symmetric matrices.
    Triangle triangle =
        switch (name) {
          case "UPPER_ROW", "LOWER_COL" -> new Triangle(true, false);
          case "LOWER_ROW", "UPPER_COL" -> new Triangle(false, false);
          case "UPPER_DIAG_ROW", "LOWER_DIAG_COL" -> new Triangle(true, true);
          case "LOWER_DIAG_ROW", "UPPER_DIAG_COL" -> new Triangle(false, true);
          default ->
              throw execution.failure(ErrorCode.LAYOUT, "unknown matrix layout " + quote(name));
        };
    if ((long) n * n > Integer.MAX_VALUE) {
      throw execution.failure(ErrorCode.SIZE_LIMIT, "matrix is too large");
    }
    Leaves stored = new Leaves(listType(domain.kind()));
    for (int i = 0; i < n; i++) {
      for (int j = triangle.from(i); j < triangle.to(n, i); j++) {
        push(domain, stored);
      }
    }
    add(stored, offset);
    Leaves full = Leaves.zeros(stored.type(), n * n);
    int k = 0;
    for (int i = 0; i < n; i++) {
      for (int j = triangle.from(i); j < triangle.to(n, i); j++) {
        full.set(i * n + j, stored, k);
        full.set(j * n + i, stored, k);
        k++;
      }
    }
    return Table.shaped(new int[] {n, n}, full);
  }

  /** The stored part of a matrix: the upper or the lower triangle, with or without the diagonal. */
  private record Triangle(boolean isUpper, boolean hasDiagonal) {
    /** Returns the first column of row {@code i}. */
    int from(int i) {
      if (!isUpper) {
        return 0;
      }
      return hasDiagonal ? i : i + 1;
    }

    /** Returns the column after the last of row {@code i} of an {@code n} by {@code n} matrix. */
    int to(int n, int i) {
      if (isUpper) {
        return n;
      }
      return hasDiagonal ? i + 1 : i;
    }
  }

  /*
   * Lists
   */

  /** Reads the list of {@code statement}, of {@code domain} (§6.9). */
  Stored list(Stmt.ListRead statement, Domain domain) {
    Leaves leaves = new Leaves(listType(statement.type().kind()));
    Expr sentinel = statement.sentinel();
    try {
      if (sentinel == null) {
        listLine(domain, leaves);
      } else {
        listUntil(domain, sentinel, leaves);
      }
    } catch (ShapeError error) {
      throw execution.failure(error, statement.name());
    }
    Expr offset = statement.offset();
    add(leaves, offset == null ? 0 : evaluator.offset(offset));
    return Table.row(leaves);
  }

  /** Reads the values on the rest of the line, or the next line if this one is used up. */
  private void listLine(Domain domain, Leaves leaves) {
    if (!input.skipToLineData() && !input.skipToData()) {
      throw execution.failure(
          ErrorCode.EXPECTED_VALUE, "expected a line of values, found end of input");
    }
    while (input.skipToLineData()) {
      push(domain, leaves);
    }
  }

  private static boolean isSameNumber(Stored x, Stored y) {
    if (x instanceof Stored.Int a && y instanceof Stored.Int b) {
      return a.value() == b.value();
    }
    if (x.type() == ValueType.STRING || y.type() == ValueType.STRING) {
      return false;
    }
    return x.asDouble() == y.asDouble();
  }

  /** Reads integers up to the terminator {@code stop}, which need not be in the range. */
  private void intsUntil(Domain domain, long stop, Leaves leaves) {
    while (true) {
      long value = intScalar(domain);
      if (value == stop) {
        return;
      }
      if (!domain.contains(value)) {
        throw outside(domain, tokenStart, input.position());
      }
      leaves.add(value);
    }
  }

  private void listUntil(Domain domain, Expr sentinel, Leaves leaves) {
    ReadingKind kind = domain.kind();
    if (kind == ReadingKind.INT && sentinel instanceof Expr.IntLiteral literal) {
      intsUntil(domain, literal.value(), leaves);
      return;
    }
    Stored stop = evaluator.evaluate(sentinel);
    if (stop.depth() > 0) {
      throw execution.failure(ErrorCode.LIST_TERMINATOR, "list terminator must be a single value");
    }
    // Words end at the token spelling an integer terminator.
    if (kind == ReadingKind.WORD && stop instanceof Stored.Int number) {
      stop = new Stored.Text(Long.toString(number.value()));
    }
    if (kind == ReadingKind.WORD && stop instanceof Stored.Real) {
      throw execution.failure(
          ErrorCode.LIST_TERMINATOR, "a list of words needs a string or integer terminator");
    }
    if (kind == ReadingKind.INT && stop instanceof Stored.Int number) {
      intsUntil(domain, number.value(), leaves);
      return;
    }
    String stopText = stop instanceof Stored.Text text ? text.value() : null;
    while (true) {
      // A string terminator is compared before the token is read as a value, so "EOF" can end a
      // list of numbers.
      if (stopText != null) {
        if (!input.skipToData()) {
          throw execution.failure(
              ErrorCode.EXPECTED_TOKEN, "expected " + quote(stopText) + ", found end of input");
        }
        if (input.isToken(stopText)) {
          input.moveOnLine(input.tokenEnd());
          return;
        }
      }
      // The terminator need not be in the range.
      Stored value = scalar(domain);
      if (isSameNumber(value, stop)) {
        return;
      }
      if (!domain.fits(value)) {
        throw outside(domain, tokenStart, input.position());
      }
      leaves.push(value);
    }
  }

  /*
   * Alphabets
   */

  /** Reads a {@code rows} by {@code columns} array of the symbols of {@code alphabet} (§6.10). */
  Stored symbols(ReadingType.Alphabet alphabet, int rows, int columns) {
    if ((long) rows * columns > Integer.MAX_VALUE) {
      throw execution.failure(ErrorCode.SIZE_LIMIT, "array is too large");
    }
    if (columns == 0) {
      execution.chargeIdle(rows);
    }
    int[] table = symbolTables.computeIfAbsent(alphabet, ValueReader::symbolTable);
    Leaves leaves = new Leaves(ValueType.INT);
    for (int row = 0; row < rows; row++) {
      for (int column = 0; column < columns; column++) {
        leaves.add(symbol(alphabet, table, row, column));
      }
    }
    // The structure is made after reading, as for arrays.
    return Table.shaped(new int[] {rows, columns}, leaves);
  }

  private long symbol(ReadingType.Alphabet alphabet, int[] table, int row, int column) {
    while (true) {
      input.skip();
      if (input.isAtEnd()) {
        throw execution.failure(
            ErrorCode.SYMBOL,
            "expected a symbol for row "
                + (row + 1)
                + ", column "
                + (column + 1)
                + ", found end of input");
      }
      if (table[input.current()] != STRIPPED) {
        break;
      }
      input.advance(input.position() + 1);
    }
    int entry = table[input.current()];
    if (entry == UNKNOWN) {
      String found = input.string(input.position(), input.position() + 1);
      throw execution.failure(
          ErrorCode.SYMBOL,
          "unexpected symbol "
              + quote(found)
              + ", expected one of "
              + quote(alphabet.symbols() + alphabet.blank()));
    }
    long value = 0;
    if (entry != BLANK) {
      try {
        value = Math.addExact(entry, alphabet.base());
      } catch (ArithmeticException _) {
        throw execution.failure(
            ErrorCode.INTEGER_OVERFLOW, "integer overflow in the value of a symbol");
      }
    }
    input.advance(input.position() + 1);
    return value;
  }

  private static int[] symbolTable(ReadingType.Alphabet alphabet) {
    int[] table = new int[256];
    Arrays.fill(table, UNKNOWN);
    byte[] symbols = alphabet.symbols().getBytes(StandardCharsets.UTF_8);
    for (int k = symbols.length - 1; k >= 0; k--) {
      table[symbols[k] & 0xff] = k;
    }
    for (byte symbol : alphabet.blank().getBytes(StandardCharsets.UTF_8)) {
      table[symbol & 0xff] = BLANK;
    }
    for (byte symbol : alphabet.strip().getBytes(StandardCharsets.UTF_8)) {
      table[symbol & 0xff] = STRIPPED;
    }
    return table;
  }
}
