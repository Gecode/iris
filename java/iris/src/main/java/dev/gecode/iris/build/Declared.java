package dev.gecode.iris.build;

import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

/**
 * A type with the dimensions and options of a declaration.
 *
 * @param type the type
 * @param dimensions the size expressions, outermost first
 * @param layout the {@code layout} expression
 * @param offset the {@code offset} expression
 */
record Declared(
    Declared.TypeSpec type,
    List<List<Op>> dimensions,
    @Nullable List<Op> layout,
    @Nullable List<Op> offset) {

  Declared(TypeSpec type) {
    this(type, List.of(), null, null);
  }

  Declared withDimensions(List<List<Op>> added) {
    List<List<Op>> all = new ArrayList<>(dimensions);
    all.addAll(added);
    return new Declared(type, List.copyOf(all), layout, offset);
  }

  Declared withLayout(List<Op> value) {
    return new Declared(type, dimensions, value, offset);
  }

  Declared withOffset(List<Op> value) {
    return new Declared(type, dimensions, layout, value);
  }

  /** Whether the text of this type ends in {@code ..}, with no upper bound. */
  boolean isOpenEnded() {
    return layout == null
        && offset == null
        && type instanceof RangeSpec range
        && range.upper == null;
  }

  /** Every expression and named type this declaration mentions. */
  void mentions(List<Mention> out) {
    for (List<Op> dimension : dimensions) {
      out.add(new Mention.Expression(dimension));
    }
    switch (type) {
      case NamedSpec named -> out.add(new Mention.TypeUse(named));
      case RangeSpec range -> {
        if (range.lower != null) {
          out.add(new Mention.Expression(range.lower));
        }
        if (range.upper != null) {
          out.add(new Mention.Expression(range.upper));
        }
      }
      case BaseSpec base -> {}
      case AlphabetSpec alphabet -> {}
    }
    if (layout != null) {
      out.add(new Mention.Expression(layout));
    }
    if (offset != null) {
      out.add(new Mention.Expression(offset));
    }
  }

  /** A type without dimensions or options. */
  sealed interface TypeSpec {}

  /** {@code int}, {@code float}, {@code num}, {@code word} or {@code rest}. */
  record BaseSpec(String name) implements TypeSpec {}

  record AlphabetSpec(String symbols, List<AlphabetOption> options) implements TypeSpec {}

  /** A use of the named type that the statement at index {@code declaration} declares. */
  record NamedSpec(String name, long owner, int declaration) implements TypeSpec {}

  record RangeSpec(@Nullable List<Op> lower, @Nullable List<Op> upper, boolean inclusive)
      implements TypeSpec {}

  sealed interface AlphabetOption {}

  record AlphabetBlank(String characters) implements AlphabetOption {}

  record AlphabetStrip(String characters) implements AlphabetOption {}

  record AlphabetBase(long value) implements AlphabetOption {}
}
