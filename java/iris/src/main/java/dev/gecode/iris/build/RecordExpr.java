package dev.gecode.iris.build;

/**
 * One record: selected by index, or a comprehension's variable. {@code record.get(field)} looks up
 * one of its fields.
 */
public final class RecordExpr {
  final Node node;

  RecordExpr(Node node) {
    this.node = node;
  }

  /**
   * {@return {@code record.name}}
   *
   * @param field the handle of the field
   */
  public IntExpr get(IntField<?> field) {
    return new Exprs.IntImpl(Exprs.member(node, field), true);
  }

  /**
   * {@return {@code record.name}}
   *
   * @param field the handle of the field
   */
  public FloatExpr get(FloatField<?> field) {
    return new Exprs.FloatImpl(Exprs.member(node, field), true);
  }

  /**
   * {@return {@code record.name}}
   *
   * @param field the handle of the field
   */
  public NumExpr get(NumField<?> field) {
    return new Exprs.NumImpl(Exprs.member(node, field), true);
  }

  /**
   * {@return {@code record.name}}
   *
   * @param field the handle of the field
   */
  public StringExpr get(StringField<?> field) {
    return new Exprs.StringImpl(Exprs.member(node, field), true);
  }

  /**
   * {@return {@code record.name}}
   *
   * @param field the handle of the field
   */
  public DynExpr get(DynField field) {
    return new Exprs.DynImpl(Exprs.member(node, field));
  }

  /**
   * {@return {@code record[index]}, for a record that is itself a list}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(IntegralExpr index) {
    return new RecordExpr(Exprs.index(node, Exprs.node(index)));
  }

  /**
   * {@return {@code record[index]}, for a record that is itself a list}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(long index) {
    return new RecordExpr(Exprs.index(node, Exprs.literal(index)));
  }

  /**
   * {@return the records of the nested repeat {@code inner} in this record}
   *
   * @param inner the record name of a repeat inside this record's repeat
   */
  public RecordList records(RecordName inner) {
    return new RecordList(new Node(new Op.ListMemberOp(inner), node));
  }
}
