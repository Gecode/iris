package dev.gecode.iris.internal.read;

/**
 * A position in the input, with its line.
 *
 * @param position the offset in bytes
 * @param line the line, from 1
 * @param lineStart the offset where the line starts
 */
record Place(int position, int line, int lineStart) {
  /** Returns the column, from 1, in bytes (§4.6). */
  int column() {
    return position - lineStart + 1;
  }
}
