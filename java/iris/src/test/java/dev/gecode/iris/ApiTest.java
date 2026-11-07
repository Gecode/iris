package dev.gecode.iris;

import static java.nio.charset.StandardCharsets.UTF_8;
import static java.util.Objects.requireNonNull;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.gecode.iris.build.Iris;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.lang.module.ModuleDescriptor;
import java.lang.module.ModuleFinder;
import java.lang.reflect.Executable;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.net.URISyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.jspecify.annotations.NullMarked;
import org.junit.jupiter.api.Test;

/** The shape of the exported package, and the parts of it that no other test reaches. */
class ApiTest {
  private static final String PACKAGE = "dev.gecode.iris";
  private static final String BUILD = PACKAGE + ".build";
  private static final Set<String> EXPORTED = Set.of(PACKAGE, BUILD);

  private static Path classes() throws URISyntaxException {
    return Path.of(Spec.class.getProtectionDomain().getCodeSource().getLocation().toURI());
  }

  private static ModuleDescriptor module() throws URISyntaxException {
    return ModuleFinder.of(classes()).find(PACKAGE).orElseThrow().descriptor();
  }

  /** Returns every public class of the exported packages, nested ones included. */
  private static List<Class<?>> publicClasses()
      throws IOException, URISyntaxException, ClassNotFoundException {
    List<Class<?>> found = new ArrayList<>();
    for (String exported : EXPORTED) {
      try (Stream<Path> files = Files.list(classes().resolve(exported.replace('.', '/')))) {
        for (Path file : files.toList()) {
          String name = file.getFileName().toString();
          if (name.endsWith(".class") && !name.equals("package-info.class")) {
            String binary = name.substring(0, name.length() - ".class".length());
            Class<?> candidate = Class.forName(exported + "." + binary);
            if (Modifier.isPublic(candidate.getModifiers())) {
              found.add(candidate);
            }
          }
        }
      }
    }
    return found;
  }

  private static IrisException failureOf(String spec, String input, Params params) {
    Spec parsed = Spec.parse(spec, "spec");
    return assertThrows(IrisException.class, () -> parsed.read(input, "input", params));
  }

  private static Value valueOf(String spec, String input) {
    return Spec.parse(spec).read(input).get("a");
  }

  @Test
  void theModuleExportsOnlyThePublicPackages() throws URISyntaxException {
    ModuleDescriptor module = module();
    assertEquals(
        EXPORTED,
        module.exports().stream()
            .map(ModuleDescriptor.Exports::source)
            .collect(Collectors.toSet()));
    assertTrue(module.exports().stream().noneMatch(ModuleDescriptor.Exports::isQualified));
    assertTrue(module.opens().isEmpty());
    for (String name : module.packages()) {
      assertTrue(EXPORTED.contains(name) || name.startsWith(PACKAGE + ".internal."), name);
    }
  }

  @Test
  void theModuleRequiresOnlyTheBaseAndStaticJspecify() throws URISyntaxException {
    Map<String, ModuleDescriptor.Requires> requires =
        module().requires().stream()
            .collect(Collectors.toMap(ModuleDescriptor.Requires::name, r -> r));
    assertEquals(Set.of("java.base", "org.jspecify"), requires.keySet());
    assertTrue(
        requireNonNull(requires.get("org.jspecify"))
            .modifiers()
            .contains(ModuleDescriptor.Requires.Modifier.STATIC));
  }

  @Test
  void theExportedPackagesAreNullMarked() {
    assertTrue(Spec.class.getPackage().isAnnotationPresent(NullMarked.class));
    assertTrue(Iris.class.getPackage().isAnnotationPresent(NullMarked.class));
  }

  /** Checks erased parameter and return types; type arguments are not inspected. */
  @Test
  void signaturesNameOnlyPublicTypes() throws Exception {
    for (Class<?> type : publicClasses()) {
      List<Executable> members = new ArrayList<>(List.of(type.getDeclaredConstructors()));
      members.addAll(List.of(type.getDeclaredMethods()));
      for (Executable member : members) {
        if (Modifier.isPublic(member.getModifiers()) && !member.isSynthetic()) {
          List<Class<?>> named = new ArrayList<>(List.of(member.getParameterTypes()));
          if (member instanceof Method method) {
            named.add(method.getReturnType());
          }
          named.forEach(used -> assertPublicType(used, member));
        }
      }
    }
  }

  private static void assertPublicType(Class<?> used, Executable member) {
    Class<?> element = used.isArray() ? used.componentType() : used;
    assertTrue(
        element.isPrimitive()
            || element.getPackageName().startsWith("java.")
            || EXPORTED.contains(element.getPackageName()),
        member.toString());
  }

  @Test
  void valuesAndErrorsAreSealed() {
    assertTrue(Value.class.isSealed());
    assertTrue(IrisError.class.isSealed());
    assertEquals(4, Value.class.getPermittedSubclasses().length);
    assertEquals(3, IrisError.class.getPermittedSubclasses().length);
  }

  @Test
  void anExceptionReportsTheInputLocation() {
    IrisException failure = failureOf("a: int", "x", Params.of());
    assertEquals(ErrorCode.EXPECTED_VALUE, failure.code());
    assertEquals("input", failure.origin());
    assertEquals(1, failure.line());
    assertEquals(1, failure.column());
    assertNull(failure.path());
    assertInstanceOf(IrisError.ReadError.class, failure.error());
  }

  @Test
  void anExceptionFallsBackOnTheSpecLocation() {
    var failure = assertThrows(IrisException.class, () -> Spec.parse("a: int\nlet", "spec"));
    assertEquals(ErrorCode.SPEC_SYNTAX, failure.code());
    assertEquals("spec", failure.origin());
    assertEquals(2, failure.line());
    assertNull(failure.column());
    assertNull(failure.path());
  }

  @Test
  void anExceptionReportsOnlyThePartsItHas() {
    IrisException failure = failureOf("param k = 1", "", Params.of("z", "1"));
    assertEquals("spec", failure.origin());
    assertNull(failure.line());
    assertNull(failure.column());
    var unnamed = assertThrows(IrisException.class, () -> Spec.parse("let"));
    assertNull(unnamed.origin());
    assertEquals(1, unnamed.line());
  }

  @Test
  void anExceptionWithoutALocationIsJustItsMessage() {
    var error = new IrisError.ReadError(ErrorCode.BUDGET, "spent", null, null, null);
    var failure = new IrisException(error);
    assertEquals("spent", failure.getMessage());
    assertEquals(error, failure.error());
    assertNull(failure.origin());
    assertNull(failure.line());
    assertNull(failure.column());
    assertNull(failure.path());
  }

  @Test
  void anExceptionOfAFailedCheckHasThePath() {
    IrisException failure = failureOf("repeat 2 {\na: int\n}\ncheck a > 0", "1 0", Params.of());
    assertEquals(List.of(1), failure.path());
    assertEquals("input", failure.origin());
    var error = assertInstanceOf(IrisError.CheckError.class, failure.error());
    assertEquals(List.of(1), error.recordPath());
    assertEquals(new Location("spec", 4, null), error.specLocation());
    List<Integer> path = requireNonNull(failure.path());
    assertThrows(UnsupportedOperationException.class, () -> path.add(0));
  }

  @Test
  void aSpecErrorHasOneLocation() {
    var failure = assertThrows(IrisException.class, () -> Spec.parse("let", "spec"));
    var error = assertInstanceOf(IrisError.SpecError.class, failure.error());
    assertEquals(new Location("spec", 1, null), error.location());
    assertEquals(error.location(), error.specLocation());
    assertEquals("expected a name, found end of spec", error.message());
  }

  @Test
  void anExceptionSurvivesSerialization() throws IOException, ClassNotFoundException {
    IrisException failure = failureOf("repeat 2 {\na: int\n}\ncheck a > 0", "1 0", Params.of());
    var bytes = new ByteArrayOutputStream();
    try (var out = new ObjectOutputStream(bytes)) {
      out.writeObject(failure);
    }
    try (var in = new ObjectInputStream(new ByteArrayInputStream(bytes.toByteArray()))) {
      IrisException copy = assertInstanceOf(IrisException.class, in.readObject());
      assertEquals(failure.error(), copy.error());
    }
  }

  @Test
  void everyParseOverloadKeepsItsOrigin() {
    byte[] bytes = "let".getBytes(UTF_8);
    assertNull(assertThrows(IrisException.class, () -> Spec.parse(bytes)).origin());
    assertNull(assertThrows(IrisException.class, () -> Spec.parse("let")).origin());
    assertEquals("s", assertThrows(IrisException.class, () -> Spec.parse(bytes, "s")).origin());
    assertEquals("s", assertThrows(IrisException.class, () -> Spec.parse("let", "s")).origin());
  }

  @Test
  void aSpecWithoutAFormatHasNoName() {
    assertNull(Spec.parse("a: int").format());
  }

  @Test
  void errorCodesSpellTheirNames() {
    for (ErrorCode code : ErrorCode.values()) {
      assertEquals("E-" + code.name().replace('_', '-'), code.code());
    }
  }

  @Test
  void valueTypesHaveTheirCanonicalNames() {
    assertEquals(
        List.of("int", "float", "string"),
        Stream.of(ValueType.values()).map(ValueType::text).toList());
  }

  @Test
  void singleValuesAreRecords() {
    assertEquals(1, new Value.IntValue(1).value());
    assertEquals(0.5, new Value.FloatValue(0.5).value());
    assertEquals("x", new Value.StringValue("x").value());
    assertEquals(ValueType.INT, new Value.IntValue(1).type());
    assertEquals(ValueType.FLOAT, new Value.FloatValue(0.5).type());
    assertEquals(ValueType.STRING, new Value.StringValue("x").type());
    assertEquals(new Value.IntValue(1), new Value.IntValue(1).at());
  }

  @Test
  void arraysWithTheSameContentAreEqual() {
    String ragged = "repeat 3 {\nn: int\na[n]: int\n}";
    Value first = valueOf(ragged, "2 1 2 0 1 3");
    Value second = valueOf(ragged, "2 1 2 0 1 3");
    assertEquals(first, second);
    assertEquals(first.hashCode(), second.hashCode());
    assertEquals(first.get(2), second.get(2));
    assertEquals(first.get(2).hashCode(), second.get(2).hashCode());
    assertEquals(first.get(1), first.get(1));
  }

  @Test
  void aRowEqualsTheSameArrayElsewhere() {
    Value row = valueOf("a[2][2]: int", "1 2 3 4").get(1);
    Value whole = valueOf("a[2]: int", "3 4");
    assertEquals(row, whole);
    assertEquals(row.hashCode(), whole.hashCode());
    Value nested = valueOf("a[2][1][2]: float", "0 0 0.5 1.5").get(1);
    Value alone = valueOf("a[1][2]: float", "0.5 1.5");
    assertEquals(nested, alone);
    assertEquals(nested.hashCode(), alone.hashCode());
  }

  @Test
  void arraysThatDifferAreNotEqual() {
    String ragged = "repeat 2 {\nn: int\na[n]: int\n}";
    assertNotEquals(valueOf(ragged, "1 1 2 2 3"), valueOf(ragged, "2 1 2 1 3"));
    assertNotEquals(valueOf("a[2]: int", "1 2"), valueOf("a[2]: int", "1 3"));
    assertNotEquals(valueOf("a[2]: int", "1 2"), valueOf("a[2]: float", "1 2"));
    assertNotEquals(valueOf("a[0]: int", ""), valueOf("a[0]: float", ""));
    assertNotEquals(valueOf("a[1][1]: int", "1"), valueOf("a[1]: int", "1"));
    assertNotEquals(valueOf("a[2]: word", "x y"), valueOf("a[2]: word", "x z"));
    assertNotEquals(valueOf("a[1]: int", "1"), new Value.IntValue(1));
  }

  @Test
  void entriesCompareByContent() {
    Data first = Spec.parse("a[2]: int").read("1 2");
    Data second = Spec.parse("a[2]: int").read("1 2");
    assertEquals(first.entries(), second.entries());
    assertEquals(first.entries().hashCode(), second.entries().hashCode());
    assertTrue(first.entries().getFirst().toString().contains("[1,2]"));
  }

  @Test
  void entriesPairNamesWithTheirValues() {
    List<Data.Entry> entries = Spec.parse("a: int\nb[1]: float").read("1 2").entries();
    assertEquals(List.of("a", "b"), entries.stream().map(Data.Entry::name).toList());
    assertEquals(new Value.IntValue(1), entries.getFirst().value());
    assertEquals(valueOf("a[1]: float", "2"), entries.getLast().value());
  }

  @Test
  void arraysReportTheTypeOfTheirLeaves() {
    assertEquals(ValueType.INT, valueOf("a[1]: int", "1").type());
    assertEquals(ValueType.FLOAT, valueOf("a[1][1]: float", "1").type());
    assertEquals(ValueType.STRING, valueOf("a[1]: word", "x").type());
  }

  @Test
  void comparesValuesOfManyDimensions() {
    int depth = 50_000;
    String spec = "a" + "[1]".repeat(depth) + ": int";
    Value first = valueOf(spec, "7");
    Value second = valueOf(spec, "7");
    assertEquals(first, second);
    assertEquals(first.hashCode(), second.hashCode());
    assertNotEquals(first, valueOf(spec, "8"));
  }
}
