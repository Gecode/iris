package dev.gecode.iris.build;

/** A clause of {@code sections}: {@link Iris#until} or {@link Iris#skipOther}. */
public sealed interface SectionClause permits Until, SkipOther {}
