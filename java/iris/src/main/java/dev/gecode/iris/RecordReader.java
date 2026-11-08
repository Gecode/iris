package dev.gecode.iris;

import dev.gecode.iris.internal.access.ViewTypes;
import dev.gecode.iris.internal.syntax.OutputEntry;
import dev.gecode.iris.internal.syntax.OutputEntry.Shape;
import java.io.UncheckedIOException;
import java.lang.annotation.Annotation;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import org.jspecify.annotations.Nullable;

/**
 * Reads data into records of type {@code R}. {@link Spec#reader} makes one and checks the record's
 * components against the entries of the reader spec, so a read can fail only as {@link
 * Spec#read(byte[])} does.
 *
 * <p>A reader is immutable, so threads can share it.
 *
 * @param <R> the record type
 */
public final class RecordReader<R extends Record> {
  private final Spec spec;
  private final Class<R> type;
  private final List<Component> components;

  /** The canonical constructor of {@code R}. */
  private final MethodHandle constructor;

  private RecordReader(
      Spec spec, Class<R> type, List<Component> components, MethodHandle constructor) {
    this.spec = spec;
    this.type = type;
    this.components = components;
    this.constructor = constructor;
  }

  /**
   * What a component reads.
   *
   * @param entry the name of the entry
   * @param view the value type and depth that the component reads the entry as, or null if it reads
   *     the entry as a {@link Value}
   * @param nullable whether an absent entry reads as null
   */
  private record Component(String entry, Shape.@Nullable Typed view, boolean nullable) {}

  /** Checks the components of {@code type} against {@code entries}, as {@link Spec#reader} says. */
  static <R extends Record> RecordReader<R> create(
      Spec spec, Class<R> type, MethodHandles.Lookup lookup, List<OutputEntry> entries) {
    if (!type.isRecord()) {
      throw new IllegalArgumentException(type.getName() + " is not a record class");
    }
    Map<String, OutputEntry> entryByName = new HashMap<>();
    for (OutputEntry entry : entries) {
      entryByName.put(entry.name(), entry);
    }
    RecordComponent[] parts = type.getRecordComponents();
    Annotation[][] parameterAnnotations = canonicalParameterAnnotations(type);
    List<Component> components = new ArrayList<>(parts.length);
    for (int i = 0; i < parts.length; i++) {
      components.add(component(parts[i], parameterAnnotations[i], entryByName));
    }
    return new RecordReader<>(
        spec, type, List.copyOf(components), canonicalConstructor(type, lookup));
  }

  private static Component component(
      RecordComponent part,
      Annotation[] parameterAnnotations,
      Map<String, OutputEntry> entryByName) {
    Entry renamed = part.getAnnotation(Entry.class);
    String name = renamed == null ? part.getName() : renamed.value();
    OutputEntry entry = entryByName.get(name);
    if (entry == null) {
      throw new IllegalArgumentException(
          describe(part) + " reads the entry '" + name + "', which no read of the spec produces");
    }
    boolean nullable =
        !part.getType().isPrimitive() && isAnnotatedNullable(part, parameterAnnotations);
    if (entry.possiblyAbsent() && !nullable) {
      throw mismatch(part, "the entry '" + name + "' may be absent, so the type needs @Nullable");
    }
    if (part.getGenericType() == Value.class) {
      return new Component(name, null, nullable);
    }
    return switch (entry.shape()) {
      case Shape.Open _ ->
          throw mismatch(
              part,
              "the entry '" + name + "' has no single value type and depth, so it reads as Value");
      case Shape.Typed view -> {
        if (!ViewTypes.isViewType(part.getGenericType(), view.leaf(), view.depth())) {
          throw mismatch(
              part,
              "the entry '"
                  + name
                  + "' is "
                  + view.leaf().text()
                  + " at depth "
                  + view.depth()
                  + ", so it reads as "
                  + ViewTypes.name(view.leaf(), view.depth())
                  + " or Value");
        }
        yield new Component(name, view, nullable);
      }
    };
  }

  /**
   * Tests whether {@code part} or its type is annotated {@code Nullable}: JSpecify's annotation, or
   * any other of that simple name that the class file keeps for run time. A declaration annotation
   * that cannot target a record component is propagated to the accessor, the field or the canonical
   * constructor's parameter instead, so those are checked too.
   */
  private static boolean isAnnotatedNullable(
      RecordComponent part, Annotation[] parameterAnnotations) {
    return Arrays.stream(part.getAnnotatedType().getAnnotations())
            .anyMatch(RecordReader::isNullable)
        || Arrays.stream(part.getAnnotations()).anyMatch(RecordReader::isNullable)
        || Arrays.stream(part.getAccessor().getAnnotations()).anyMatch(RecordReader::isNullable)
        || Arrays.stream(field(part).getAnnotations()).anyMatch(RecordReader::isNullable)
        || Arrays.stream(parameterAnnotations).anyMatch(RecordReader::isNullable);
  }

  /** Returns the annotations of each parameter of the canonical constructor of {@code type}. */
  private static Annotation[][] canonicalParameterAnnotations(Class<?> type) {
    try {
      return type.getDeclaredConstructor(componentTypes(type)).getParameterAnnotations();
    } catch (NoSuchMethodException e) {
      throw new IllegalStateException("a record class has a canonical constructor", e);
    }
  }

  private static Class<?>[] componentTypes(Class<?> type) {
    return Arrays.stream(type.getRecordComponents())
        .map(RecordComponent::getType)
        .toArray(Class<?>[]::new);
  }

  /** Returns the private field that holds the value of {@code part}. */
  private static java.lang.reflect.Field field(RecordComponent part) {
    try {
      return part.getDeclaringRecord().getDeclaredField(part.getName());
    } catch (NoSuchFieldException e) {
      throw new IllegalStateException("a record has a field for each component", e);
    }
  }

  private static boolean isNullable(Annotation annotation) {
    return annotation.annotationType().getSimpleName().equals("Nullable");
  }

  private static IllegalArgumentException mismatch(RecordComponent part, String reason) {
    return new IllegalArgumentException(
        describe(part) + " has type " + typeName(part.getGenericType()) + ", but " + reason);
  }

  private static String describe(RecordComponent part) {
    return "the component '" + part.getName() + "' of " + part.getDeclaringRecord().getSimpleName();
  }

  /** Names {@code type} with simple names, as messages compare it with {@link ViewTypes#name}. */
  private static String typeName(Type type) {
    return switch (type) {
      case Class<?> simple -> simple.getSimpleName();
      case ParameterizedType generic ->
          typeName(generic.getRawType())
              + Arrays.stream(generic.getActualTypeArguments())
                  .map(RecordReader::typeName)
                  .collect(Collectors.joining(", ", "<", ">"));
      default -> type.getTypeName();
    };
  }

  private static MethodHandle canonicalConstructor(Class<?> type, MethodHandles.Lookup lookup) {
    try {
      return lookup.findConstructor(type, MethodType.methodType(void.class, componentTypes(type)));
    } catch (IllegalAccessException e) {
      throw new IllegalArgumentException(
          "the lookup cannot access the canonical constructor of " + type.getName(), e);
    } catch (NoSuchMethodException e) {
      throw new IllegalStateException("a record class has a canonical constructor", e);
    }
  }

  /**
   * Reads an input without an origin or parameters.
   *
   * @param input the bytes of the input
   * @return the record of the data read
   * @throws IrisException for the first error
   */
  public R read(byte[] input) {
    return construct(spec.read(input));
  }

  /**
   * Reads an input without parameters.
   *
   * @param input the bytes of the input
   * @param origin the name of the input in errors
   * @return the record of the data read
   * @throws IrisException for the first error
   */
  public R read(byte[] input, String origin) {
    return construct(spec.read(input, origin));
  }

  /**
   * Reads an input.
   *
   * @param input the bytes of the input
   * @param origin the name of the input in errors
   * @param params the parameters
   * @return the record of the data read
   * @throws IrisException for the first error
   */
  public R read(byte[] input, String origin, Params params) {
    return construct(spec.read(input, origin, params));
  }

  /**
   * Reads an input without an origin or parameters.
   *
   * @param input the text of the input
   * @return the record of the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public R read(String input) {
    return construct(spec.read(input));
  }

  /**
   * Reads an input without parameters.
   *
   * @param input the text of the input
   * @param origin the name of the input in errors
   * @return the record of the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public R read(String input, String origin) {
    return construct(spec.read(input, origin));
  }

  /**
   * Reads an input.
   *
   * @param input the text of the input
   * @param origin the name of the input in errors
   * @param params the parameters
   * @return the record of the data read
   * @throws IrisException for the first error
   * @throws IllegalArgumentException if the text has an unpaired surrogate, which has no UTF-8 form
   */
  public R read(String input, String origin, Params params) {
    return construct(spec.read(input, origin, params));
  }

  /**
   * Reads a file without parameters. Its path is the origin of the input.
   *
   * @param input the file
   * @return the record of the data read
   * @throws IrisException for the first error
   * @throws UncheckedIOException if the file cannot be read
   */
  public R read(Path input) {
    return construct(spec.read(input));
  }

  /**
   * Reads a file. Its path is the origin of the input.
   *
   * @param input the file
   * @param params the parameters
   * @return the record of the data read
   * @throws IrisException for the first error
   * @throws UncheckedIOException if the file cannot be read
   */
  public R read(Path input, Params params) {
    return construct(spec.read(input, params));
  }

  /** Passes each component its entry, or null for an absent entry, to the canonical constructor. */
  private R construct(Data data) {
    @Nullable Object[] arguments = new Object[components.size()];
    for (int k = 0; k < arguments.length; k++) {
      Component component = components.get(k);
      Value value =
          component.nullable() ? data.find(component.entry()) : data.get(component.entry());
      Shape.Typed view = component.view();
      arguments[k] =
          value == null || view == null ? value : ViewTypes.view(value, view.leaf(), view.depth());
    }
    try {
      return type.cast(constructor.invokeWithArguments(arguments));
    } catch (RuntimeException | Error e) {
      throw e;
    } catch (Throwable e) {
      // A canonical constructor cannot declare a checked exception.
      throw new IllegalStateException(e);
    }
  }
}
