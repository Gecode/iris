package dev.gecode.iris.build;

/**
 * What a comprehension ranges over: an array expression, which gives its elements as {@code E}, or
 * the records of a repeat, which it gives as {@link RecordExpr}.
 *
 * @param <E> what the comprehension gives its body
 */
public sealed interface Domain<E>
    permits IntExpr, FloatExpr, NumExpr, StringExpr, DynExpr, RecordName, RecordList, Over {}
