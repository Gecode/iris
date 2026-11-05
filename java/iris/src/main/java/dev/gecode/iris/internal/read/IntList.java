package dev.gecode.iris.internal.read;

import java.util.Arrays;

/** A growing list of offsets. */
final class IntList {
  private int[] values = new int[2];
  private int size = 0;

  void add(int value) {
    if (size == values.length) {
      values = Arrays.copyOf(values, Leaves.grown(size));
    }
    values[size++] = value;
  }

  int last() {
    return values[size - 1];
  }

  int[] toArray() {
    return Arrays.copyOf(values, size);
  }
}
