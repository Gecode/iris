package dev.gecode.iris.build;

import static dev.gecode.iris.build.Iris.*;
import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.Data;
import dev.gecode.iris.Field;
import dev.gecode.iris.Spec;
import dev.gecode.iris.Value;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import org.jspecify.annotations.Nullable;
import org.junit.jupiter.api.Test;

/** What {@link Data#get(Field)} returns for each kind of handle. */
class ReadingTest {
  @Test
  void eachFactoryReadsItsJavaType() {
    var a = int0("a");
    var b = float1("b");
    var c = string2("c");
    var d = int3("d");
    var e = num0("e");
    var f = string0("f");
    Spec spec =
        spec(() -> {
          read(a, intType());
          read(b, floatType().dim(2));
          read(c, word().dims(2, 1));
          read(d, intType().dims(1, 2, 1));
          read(e, numType());
          read(f, word());
        });
    Data data = spec.read("1 0.5 2 x y 3 4 2.5 z");
    long first = data.get(a);
    assertEquals(1L, first);
    assertArrayEquals(new double[] {0.5, 2.0}, data.get(b));
    assertEquals(List.of(List.of("x"), List.of("y")), data.get(c));
    List<List<long[]>> deep = data.get(d);
    assertArrayEquals(new long[] {3}, deep.getFirst().getFirst());
    assertArrayEquals(new long[] {4}, deep.getFirst().getLast());
    assertEquals(2.5, data.get(e));
    assertEquals("z", data.get(f));
  }

  @Test
  void anEmptyArrayHasTheTypedEmptyView() {
    var a = int1("a");
    var b = string1("b");
    var c = float2("c");
    Spec spec =
        spec(() -> {
          read(a, intType().dim(0));
          read(b, word().dim(0));
          read(c, floatType().dims(0, 3));
        });
    Data data = spec.read("");
    assertArrayEquals(new long[0], data.get(a));
    assertEquals(List.of(), data.get(b));
    assertEquals(List.of(), data.get(c));
  }

  @Test
  void aRepeatOfNothingCollectsAnEmptyArray() {
    var x = int1("x");
    Spec spec = spec(() -> repeat(0, () -> read(x, intType())));
    assertArrayEquals(new long[0], spec.read("").get(x));
  }

  @Test
  void rowsAreUnmodifiable() {
    var m = int2("m");
    List<long[]> rows = spec(() -> read(m, intType().dims(2, 2))).read("1 2 3 4").get(m);
    assertArrayEquals(new long[] {3, 4}, rows.get(1));
    assertThrows(UnsupportedOperationException.class, () -> rows.add(new long[0]));
  }

  @Test
  void anAbsentOptionalEntryIsNull() {
    var n = int0("n");
    var x = int0("x").optional();
    Spec spec =
        spec(() -> {
          read(n, intType());
          ifThen(n.gt(0), () -> read(x, intType()));
        });
    @Nullable Long absent = spec.read("0").get(x);
    assertNull(absent);
    assertEquals(5L, spec.read("1 5").get(x));
  }

  @Test
  void numbersKeepTheTypeTheDataGave() {
    var e = num0("e");
    var row = num1("row");
    Spec spec =
        spec(() -> {
          read(e, numType());
          read(row, numType().dim(2));
        });
    Data ints = spec.read("1 2 3");
    assertEquals(1L, ints.get(e));
    assertEquals(List.of(2L, 3L), ints.get(row));
    Data floats = spec.read("1.5 2 3.5");
    assertEquals(1.5, floats.get(e));
    assertEquals(List.of(2.0, 3.5), floats.get(row));
  }

  @Test
  void dynamicHandlesReadValues() {
    var n = int0("n");
    var deep = intN("deep", 2);
    var open = dynamic("open");
    var p = dynamic("p");
    Spec spec =
        spec(() -> {
          read(n, intType());
          read(deep, intType().dims(1, 2));
          ifThen(n.gt(0), () -> read(open, word()));
          param(p);
        });
    Data data = spec.read("0 1 2", "input", dev.gecode.iris.Params.of("p", "7"));
    Value value = data.get(deep);
    assertEquals(2, value.depth());
    assertArrayEquals(new long[] {1, 2}, value.get(0).longs());
    assertNull(data.get(open));
    assertEquals(7L, requireValue(data.get(p)).asLong());
    assertEquals(
        "w",
        requireValue(spec.read("1 1 2 w", "input", dev.gecode.iris.Params.of("p", "7")).get(open))
            .asString());
  }

  private static Value requireValue(@Nullable Value value) {
    return java.util.Objects.requireNonNull(value);
  }

  @Test
  void aHandleOfAnotherSpecReportsWhatWentWrong() {
    Spec spec = spec(() -> read(float0("x"), floatType()));
    Data data = spec.read("1.5");
    var missing = assertThrows(IllegalStateException.class, () -> data.get(int0("y")));
    assertTrue(
        String.valueOf(missing.getMessage()).contains("no value named 'y'"), missing.getMessage());
    var type = assertThrows(IllegalStateException.class, () -> data.get(int0("x")));
    assertTrue(String.valueOf(type.getMessage()).contains("float at depth 0"), type.getMessage());
    var depth = assertThrows(IllegalStateException.class, () -> data.get(float1("x")));
    assertTrue(String.valueOf(depth.getMessage()).contains("float at depth 1"), depth.getMessage());
    assertNull(data.get(int0("y").optional()));
  }

  @Test
  void aReturnedHandleReadsTheEntryAsItIsNow() {
    AtomicReference<@Nullable DynField> kept = new AtomicReference<>();
    var x = int0("x");
    Spec spec =
        spec(() -> {
          read(x, intType());
          kept.set(read(x, floatType()));
        });
    DynField handle = java.util.Objects.requireNonNull(kept.get());
    @Nullable Value read = spec.read("1 2.5").get(handle);
    Value value = requireValue(read);
    assertEquals(2.5, value.asDouble());
  }

  @Test
  void anOptionalHandleOfABuilderCannotBeMade() {
    AtomicReference<@Nullable Throwable> thrown = new AtomicReference<>();
    builder(() -> {
      IntField<Long> x = forward("x", Int.class);
      read(x, intType());
      thrown.set(assertThrows(IllegalStateException.class, x::optional));
    });
    assertInstanceOf(IllegalStateException.class, thrown.get());
  }

  @Test
  void handlesPrintTheirNames() {
    assertEquals("IntField n_jobs", int0("n_jobs").toString());
    assertEquals("StringField name", string0("name").optional().toString());
    assertEquals("DynField d", dynamic("d").toString());
    assertEquals("n_jobs", int0("n_jobs").name());
  }

  @Test
  void aFieldOfAnotherClassReadsButDoesNotBuild() {
    Field<String> upper =
        new Field<>("w") {
          @Override
          protected String readFrom(Data data) {
            return data.get("w").asString().toUpperCase(java.util.Locale.ROOT);
          }
        };
    Spec spec = spec(() -> read(string0("w"), word()));
    assertEquals("ABC", spec.read("abc").get(upper));
    assertThrows(IllegalArgumentException.class, () -> spec(() -> read(upper, word())));
  }
}
