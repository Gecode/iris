package dev.gecode.iris.build;

import org.jspecify.annotations.Nullable;

/**
 * The records of a repeat. {@code repeat(count, record, body)} binds it to the repeat; statements
 * before and after it may use it.
 *
 * <p>{@code record.get(field)} is the field of the current record, {@link #list} all the records,
 * and a comprehension over a record gives its body one record at a time. Like a declared handle, a
 * record name belongs to no builder: each builder that uses it binds it to one of its repeats.
 */
public final class RecordName implements Domain<RecordExpr> {
  final @Nullable String each;
  final @Nullable String list;

  RecordName(@Nullable String each, @Nullable String list) {
    this.each = each;
    this.list = list;
  }

  private Node node() {
    return new Node(new Op.RecordOp(this));
  }

  /** How messages name this record. */
  String describe() {
    return describe(each);
  }

  static String describe(@Nullable String each) {
    return each == null ? "from `recordName()`" : "`" + each + "`";
  }

  /**
   * {@return {@code record.name}, the field of the current record}
   *
   * @param field the handle of the field
   */
  public IntExpr get(IntField<?> field) {
    return new Exprs.IntImpl(Exprs.member(node(), field), true);
  }

  /**
   * {@return {@code record.name}, the field of the current record}
   *
   * @param field the handle of the field
   */
  public FloatExpr get(FloatField<?> field) {
    return new Exprs.FloatImpl(Exprs.member(node(), field), true);
  }

  /**
   * {@return {@code record.name}, the field of the current record}
   *
   * @param field the handle of the field
   */
  public NumExpr get(NumField<?> field) {
    return new Exprs.NumImpl(Exprs.member(node(), field), true);
  }

  /**
   * {@return {@code record.name}, the field of the current record}
   *
   * @param field the handle of the field
   */
  public StringExpr get(StringField<?> field) {
    return new Exprs.StringImpl(Exprs.member(node(), field), true);
  }

  /**
   * {@return {@code record.name}, the field of the current record}
   *
   * @param field the handle of the field
   */
  public DynExpr get(DynField field) {
    return new Exprs.DynImpl(Exprs.member(node(), field));
  }

  /** {@return all the records of this repeat, printed as its {@code as} name} */
  public RecordList list() {
    return new RecordList(new Node(new Op.RecordListOp(this)));
  }

  /**
   * {@return the records of the nested repeat {@code inner} in the current record}
   *
   * @param inner the record name of a repeat inside this one
   */
  public RecordList records(RecordName inner) {
    return new RecordList(new Node(new Op.ListMemberOp(inner), node()));
  }

  /**
   * {@return one of the records, {@code records[index]}}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(IntegralExpr index) {
    return list().at(index);
  }

  /**
   * {@return one of the records, {@code records[index]}}
   *
   * @param index the index, counted from 0
   */
  public RecordExpr at(long index) {
    return list().at(index);
  }
}
