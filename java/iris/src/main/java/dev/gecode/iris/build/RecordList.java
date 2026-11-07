package dev.gecode.iris.build;

/** A list of records: all the records of a repeat, or a nested list. {@link #at} selects one. */
public final class RecordList implements Domain<RecordExpr> {
  final Node node;

  RecordList(Node node) {
    this.node = node;
  }

  /**
   * {@return {@code records[index]}}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(IntegralExpr index) {
    return new RecordExpr(Exprs.index(node, Exprs.node(index)));
  }

  /**
   * {@return {@code records[index]}}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(long index) {
    return new RecordExpr(Exprs.index(node, Exprs.literal(index)));
  }
}
