package dev.gecode.iris.build;

import java.util.ArrayList;
import java.util.List;

/**
 * {@code alphabet "symbols"}, which reads each symbol as its index. {@link #blank}, {@link #strip}
 * and {@link #base} add options in call order.
 */
public final class Alphabet extends IntType {
  Alphabet(Declared declared) {
    super(declared);
  }

  private Alphabet option(Declared.AlphabetOption option) {
    Declared.AlphabetSpec spec = (Declared.AlphabetSpec) declared.type();
    List<Declared.AlphabetOption> options = new ArrayList<>(spec.options());
    options.add(option);
    return new Alphabet(
        new Declared(
            new Declared.AlphabetSpec(spec.symbols(), List.copyOf(options)),
            declared.dimensions(),
            declared.layout(),
            declared.offset()));
  }

  /**
   * {@return this alphabet with {@code blank "characters"}}
   *
   * @param characters the characters that read as blanks
   */
  public Alphabet blank(String characters) {
    return option(new Declared.AlphabetBlank(characters));
  }

  /**
   * {@return this alphabet with {@code strip "characters"}}
   *
   * @param characters the characters to remove
   */
  public Alphabet strip(String characters) {
    return option(new Declared.AlphabetStrip(characters));
  }

  /**
   * {@return this alphabet with {@code base value}}
   *
   * @param value the index of the first symbol
   */
  public Alphabet base(long value) {
    return option(new Declared.AlphabetBase(value));
  }
}
