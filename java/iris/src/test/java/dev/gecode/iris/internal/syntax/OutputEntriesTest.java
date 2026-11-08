package dev.gecode.iris.internal.syntax;

import static java.nio.charset.StandardCharsets.UTF_8;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.ValueType;
import dev.gecode.iris.internal.syntax.OutputEntry.Facts;
import dev.gecode.iris.internal.syntax.OutputEntry.Leaf;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

/** The output entries of reader specs, as the tests of iris-sema's semantic model check them. */
class OutputEntriesTest {
  private static List<OutputEntry> entries(String source) {
    return LargeStack.call(() -> {
      Ast.Program program = Parser.parse(Lexer.lex(source.getBytes(UTF_8), "model"), "model");
      return OutputEntries.of(program, Resolver.resolve(program, "model"));
    });
  }

  private static OutputEntry entry(List<OutputEntry> entries, String name) {
    return entries.stream()
        .filter(entry -> entry.name().equals(name))
        .findFirst()
        .orElseThrow(() -> new AssertionError("no output entry '" + name + "'"));
  }

  private static List<String> names(List<OutputEntry> entries) {
    return entries.stream().map(OutputEntry::name).toList();
  }

  @Test
  void outputEntriesInsideRepeatsCollectEveryEnclosingLevel() {
    var entries = entries("repeat 2 {n:int repeat n {a[n]:float list b:word until eol}}");
    assertEquals(List.of("n", "a", "b"), names(entries));
    assertEquals(Facts.known(ValueType.INT, 1), entry(entries, "n").facts());
    assertEquals(Facts.known(ValueType.FLOAT, 3), entry(entries, "a").facts());
    assertEquals(Facts.known(ValueType.STRING, 3), entry(entries, "b").facts());
    assertEquals(new Shape.Typed(Leaf.FLOAT, 3), entry(entries, "a").shape());
    assertFalse(entry(entries, "a").possiblyAbsent());
  }

  @Test
  void anEmptyRepeatKeepsTheFallbackDepthButNotItsValueType() {
    var entries = entries("a[2]:int repeat 0 {let s=\"text\" let row=a}");
    // A `let` falls back to an empty int column, which matches any value type.
    assertEquals(Facts.known(ValueType.STRING, 1), entry(entries, "s").facts());
    assertEquals(new Shape.Typed(Leaf.STRING, 1), entry(entries, "s").shape());
    var row = entry(entries, "row");
    assertEquals(new Facts(Set.of(ValueType.INT), Set.of(2, 1)), row.facts());
    assertEquals(new Shape.Open(), row.shape());
  }

  @Test
  void outputEntriesOfConditionalDefinitionsMayBeAbsent() {
    var entries =
        entries(
            """
            c:int
            if c {x:int} else {y:int}
            if c {z:int}
            z:int
            sections {"A {a:int}" optional "B {b:int}" {let d=1}}
            if c {repeat 2 {r:int}}
            """);
    for (String name : List.of("x", "y", "a", "r")) {
      assertTrue(entry(entries, name).possiblyAbsent(), name);
    }
    for (String name : List.of("c", "z", "b", "d")) {
      assertFalse(entry(entries, name).possiblyAbsent(), name);
    }
  }

  @Test
  void aRepeatPublishesItsConditionalFields() {
    var entries =
        entries(
            """
            repeat 2 {k:int if k {list a:int until eol}
            sections {"A {b:int}" optional}}
            """);
    assertFalse(entry(entries, "a").possiblyAbsent());
    assertFalse(entry(entries, "b").possiblyAbsent());
    assertEquals(Facts.known(ValueType.INT, 2), entry(entries, "a").facts());
  }

  @Test
  void instancesAndSectionsKeepTheirDefinitionsFacts() {
    var entries =
        entries(
            """
            instances {sections {"A {n:int}" {repeat n {v:float}} "B {t:word}" optional}
            let m=1}
            """);
    assertEquals(Facts.known(ValueType.INT, 0), entry(entries, "n").facts());
    assertEquals(Facts.known(ValueType.FLOAT, 1), entry(entries, "v").facts());
    assertEquals(Facts.known(ValueType.STRING, 0), entry(entries, "t").facts());
    assertTrue(entry(entries, "t").possiblyAbsent());
    assertFalse(entry(entries, "n").possiblyAbsent());
    assertFalse(entry(entries, "m").possiblyAbsent());
  }

  @Test
  void outputEntriesOfARedefinedNameJoinAllItsDefinitions() {
    var entries = entries("x:int let x=\"s\" y:int repeat 2 {y:int} z:int z:int");
    assertEquals(Set.of(ValueType.INT, ValueType.STRING), entry(entries, "x").facts().types());
    assertEquals(new Shape.Open(), entry(entries, "x").shape());
    assertEquals(Set.of(0, 1), entry(entries, "y").facts().depths());
    assertEquals(Facts.known(ValueType.INT, 0), entry(entries, "z").facts());
  }

  @Test
  void numIsTypedOnlyWhenEveryDefinitionIsIntOrFloat() {
    var entries = entries("x:num y:int let y=1.5 let z=-x");
    assertEquals(new Shape.Typed(Leaf.NUM, 0), entry(entries, "x").shape());
    assertEquals(
        new Facts(Set.of(ValueType.INT, ValueType.FLOAT), Set.of(0)), entry(entries, "y").facts());
    assertEquals(new Shape.Open(), entry(entries, "y").shape());
    assertEquals(new Shape.Typed(Leaf.NUM, 0), entry(entries, "z").shape());
  }

  @Test
  void unreferencedEntriesAreOutputAndStructuralNamesAreNot() {
    var entries =
        entries(
            """
            type t = int
            v:t
            param p=2
            repeat 2 as rows {each row w:int let unused=1}
            let total=sum(i in w: i)
            """);
    assertEquals(List.of("v", "p", "w", "unused", "total"), names(entries));
  }

  @Test
  void aStoreNamedUnderscoreStaysOnlyOutsideRepeats() {
    var top = entries("repeat until \"END\" store _ {x:int}");
    assertEquals(List.of("_", "x"), names(top));
    assertEquals(Facts.known(ValueType.STRING, 0), entry(top, "_").facts());
    assertFalse(entry(top, "_").possiblyAbsent());

    var nested = entries("repeat 2 {repeat until \"END\" store _ {x:int}}");
    assertEquals(List.of("x"), names(nested));
  }

  @Test
  void aStoreOfSectionsMayBeAbsentOutsideARepeat() {
    var top = entries("sections until \"END\" store ending {\"A\" optional}");
    assertTrue(entry(top, "ending").possiblyAbsent());
    var nested = entries("repeat 2 {sections until \"END\" store ending {\"A\" optional}}");
    assertFalse(entry(nested, "ending").possiblyAbsent());
    assertEquals(Facts.known(ValueType.STRING, 1), entry(nested, "ending").facts());
  }

  @Test
  void anUnknownFactStaysUnknown() {
    var entries = entries("sections {\"A\" {let y=x} \"B {x:word}\" optional}");
    assertEquals(Facts.UNKNOWN, entry(entries, "y").facts());
    assertEquals(new Shape.Open(), entry(entries, "y").shape());
  }

  @Test
  void outputEntriesHandleAnyNestingDepth() {
    int depth = 256;
    var entries = entries("repeat 1 {".repeat(depth) + "v:int" + "}".repeat(depth));
    assertEquals(Facts.known(ValueType.INT, depth), entry(entries, "v").facts());
  }
}
