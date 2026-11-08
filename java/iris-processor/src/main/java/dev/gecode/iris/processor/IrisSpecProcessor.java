package dev.gecode.iris.processor;

import static java.nio.charset.StandardCharsets.UTF_8;

import com.sun.source.util.TreePath;
import com.sun.source.util.Trees;
import dev.gecode.iris.IrisException;
import dev.gecode.iris.Spec;
import dev.gecode.iris.internal.syntax.Ast;
import dev.gecode.iris.internal.syntax.LargeStack;
import dev.gecode.iris.internal.syntax.Lexer;
import dev.gecode.iris.internal.syntax.OutputEntries;
import dev.gecode.iris.internal.syntax.OutputEntry;
import dev.gecode.iris.internal.syntax.Parser;
import dev.gecode.iris.internal.syntax.Resolution;
import dev.gecode.iris.internal.syntax.Resolver;
import java.io.IOException;
import java.io.Writer;
import java.nio.file.Files;
import java.nio.file.InvalidPathException;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;
import javax.annotation.processing.AbstractProcessor;
import javax.annotation.processing.Messager;
import javax.annotation.processing.RoundEnvironment;
import javax.lang.model.SourceVersion;
import javax.lang.model.element.AnnotationMirror;
import javax.lang.model.element.AnnotationValue;
import javax.lang.model.element.Element;
import javax.lang.model.element.Modifier;
import javax.lang.model.element.NestingKind;
import javax.lang.model.element.PackageElement;
import javax.lang.model.element.TypeElement;
import javax.tools.Diagnostic.Kind;
import org.jspecify.annotations.Nullable;

/**
 * Generates the handles class and the record of each type annotated {@link IrisSpec}, as the
 * annotation's documentation describes.
 *
 * <p>A missing or unreadable file, a reader spec that does not parse or check, a nested type, and a
 * generated class that exists already are errors, reported at the annotation's value or at the
 * annotated type. A spec whose entries would take more than 254 parameter slots, the most that a
 * record's canonical constructor can take, gets no record and no reader, with a note; its handles
 * class is generated all the same. The generated classes are laid out as google-java-format lays
 * them out.
 *
 * <p>The processor checks the spec with {@code dev.gecode.iris} and finds its entries with that
 * library's internal port of the output entries of {@code iris gen}, so it must run from the
 * processor path, where the library is not a named module.
 */
public final class IrisSpecProcessor extends AbstractProcessor {
  /** The processor option that names the directory that relative paths resolve against. */
  public static final String ROOT_OPTION = "iris.root";

  private static final String ANNOTATION = IrisSpec.class.getCanonicalName();

  /** Creates the processor, which the compiler does through the service loader. */
  public IrisSpecProcessor() {}

  @Override
  public Set<String> getSupportedAnnotationTypes() {
    return Set.of(ANNOTATION);
  }

  @Override
  public Set<String> getSupportedOptions() {
    return Set.of(ROOT_OPTION);
  }

  @Override
  public SourceVersion getSupportedSourceVersion() {
    return SourceVersion.latestSupported();
  }

  @Override
  public boolean process(Set<? extends TypeElement> annotations, RoundEnvironment round) {
    for (Element element : round.getElementsAnnotatedWith(IrisSpec.class)) {
      if (element instanceof TypeElement type) {
        generate(type);
      }
    }
    // Claiming the annotation would hide it from other processors.
    return false;
  }

  /** Generates the classes of {@code type}, or reports why it cannot. */
  private void generate(TypeElement type) {
    Messager messager = processingEnv.getMessager();
    AnnotationMirror mirror = mirror(type);
    AnnotationValue value = value(mirror);
    if (type.getNestingKind() != NestingKind.TOP_LEVEL) {
      messager.printMessage(Kind.ERROR, "@IrisSpec applies only to a top-level type", type, mirror);
      return;
    }
    String path = type.getAnnotation(IrisSpec.class).value();
    Path file;
    try {
      file = resolve(type, path);
    } catch (InvalidPathException e) {
      messager.printMessage(
          Kind.ERROR, "'" + path + "' is not a path: " + e.getReason(), type, mirror, value);
      return;
    }
    if (file == null) {
      messager.printMessage(
          Kind.ERROR,
          "cannot resolve the relative path '"
              + path
              + "': the source file of "
              + type.getSimpleName()
              + " is unknown; set the processor option -A"
              + ROOT_OPTION
              + "=DIR",
          type,
          mirror,
          value);
      return;
    }
    byte[] source;
    try {
      source = Files.readAllBytes(file);
    } catch (IOException e) {
      messager.printMessage(
          Kind.ERROR,
          "cannot read the reader spec '" + path + "' at " + file + ": " + e,
          type,
          mirror,
          value);
      return;
    }
    List<OutputEntry> entries;
    try {
      Spec.parse(source, path);
      entries = entries(source, path);
    } catch (IrisException e) {
      messager.printMessage(
          Kind.ERROR, "the reader spec is invalid: " + e.getMessage(), type, mirror, value);
      return;
    }
    List<Handle> handles = Handle.of(entries);
    String name = type.getSimpleName().toString();
    boolean fits = Generator.slots(handles) <= Generator.MOST_RECORD_SLOTS;
    if (!fits) {
      messager.printMessage(
          Kind.NOTE,
          "the entries of '"
              + path
              + "' take more than "
              + Generator.MOST_RECORD_SLOTS
              + " parameter slots, so "
              + name
              + "Data and the reader are not generated",
          type,
          mirror,
          value);
    }
    var generator =
        new Generator(
            packageName(type),
            name + "Handles",
            fits ? name + "Data" : null,
            type.getModifiers().contains(Modifier.PUBLIC),
            path,
            type.getQualifiedName().toString(),
            new String(source, UTF_8),
            handles);
    write(
        type, mirror, generator.packageName(), generator.handlesClass(), generator.handlesSource());
    String record = generator.recordClass();
    if (record != null) {
      write(type, mirror, generator.packageName(), record, generator.recordSource());
    }
  }

  /**
   * Returns the file at {@code path}: as it is if absolute, else against the {@code iris.root}
   * option or the directory of {@code type}'s source file, or null if neither is known.
   */
  private @Nullable Path resolve(TypeElement type, String path) {
    Path given = Path.of(path);
    if (given.isAbsolute()) {
      return given;
    }
    String root = processingEnv.getOptions().get(ROOT_OPTION);
    if (root != null) {
      return Path.of(root).resolve(given);
    }
    Path directory = sourceDirectory(type);
    return directory == null ? null : directory.resolve(given);
  }

  /** Returns the directory of {@code type}'s source file, or null if the compiler cannot tell. */
  private @Nullable Path sourceDirectory(TypeElement type) {
    try {
      TreePath tree = Trees.instance(processingEnv).getPath(type);
      if (tree == null) {
        return null;
      }
      Path source = Path.of(tree.getCompilationUnit().getSourceFile().toUri());
      return source.getParent();
    } catch (IllegalArgumentException | UnsupportedOperationException e) {
      // Another compiler than javac, or a source that is not a file.
      return null;
    }
  }

  /** Returns the entries of a valid reader spec. */
  static List<OutputEntry> entries(byte[] source, String origin) {
    Ast.Program program = LargeStack.call(() -> Parser.parse(Lexer.lex(source, origin), origin));
    Resolution resolution = LargeStack.call(() -> Resolver.resolve(program, origin));
    return LargeStack.call(() -> OutputEntries.of(program, resolution));
  }

  private void write(
      TypeElement type, AnnotationMirror mirror, String packageName, String name, String source) {
    String qualified = packageName.isEmpty() ? name : packageName + "." + name;
    try (Writer writer = processingEnv.getFiler().createSourceFile(qualified, type).openWriter()) {
      writer.write(source);
    } catch (IOException e) {
      processingEnv
          .getMessager()
          .printMessage(Kind.ERROR, "cannot generate " + qualified + ": " + e, type, mirror);
    }
  }

  private String packageName(TypeElement type) {
    PackageElement element = processingEnv.getElementUtils().getPackageOf(type);
    return element.isUnnamed() ? "" : element.getQualifiedName().toString();
  }

  /** Returns the annotation on {@code type}, for messages at its position. */
  private static AnnotationMirror mirror(TypeElement type) {
    return type.getAnnotationMirrors().stream()
        .filter(m -> m.getAnnotationType().toString().equals(ANNOTATION))
        .findFirst()
        .orElseThrow();
  }

  /** Returns the value of {@code mirror}, for messages at the path's position. */
  private static AnnotationValue value(AnnotationMirror mirror) {
    return mirror.getElementValues().entrySet().stream()
        .filter(e -> e.getKey().getSimpleName().contentEquals("value"))
        .map(java.util.Map.Entry::getValue)
        .findFirst()
        .orElseThrow();
  }
}
