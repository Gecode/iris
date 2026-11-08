package dev.gecode.iris.build;

import dev.gecode.iris.Field;
import dev.gecode.iris.Spec;
import dev.gecode.iris.Value;
import dev.gecode.iris.build.BuildException.Kind;
import dev.gecode.iris.build.State.Block;
import dev.gecode.iris.build.State.BlockKind;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import org.jspecify.annotations.Nullable;

/**
 * Builds reader specs from Java code. Import its members with {@code import static
 * dev.gecode.iris.build.Iris.*;}.
 *
 * <p>Handles are typed constants that name entries: {@code int2("machine")} is an {@code
 * IntField<List<long[]>>}, and {@code data.get(machine)} returns a {@code List<long[]>}. Each
 * statement is a static method, which adds the statement to the innermost open block; a block
 * statement takes its body as a lambda, which runs with that block open. A scoped value holds the
 * open block, so a statement called outside a builder body throws {@link IllegalStateException}.
 *
 * <pre>{@code
 * static final IntField<Long> n = int0("n");
 * static final IntField<long[]> x = int1("x");
 * static final Spec spec =
 *     spec(
 *         "numbers",
 *         () -> {
 *           read(n, atLeast(0));
 *           repeat(n, () -> read(x, intType()));
 *         });
 * }</pre>
 *
 * <p>A mistake that one statement shows throws {@link BuildException} at that call; the checks of
 * the whole spec throw it from {@link Builder#build}. Either way its call site is the line of the
 * caller's code that made the statement.
 */
public final class Iris {
  private Iris() {}

  private static final ScopedValue<Block> OPEN = ScopedValue.newInstance();

  // Builders

  /**
   * Builds a reader spec: {@code body} adds its statements.
   *
   * @param body the statements
   * @return the builder, whose {@link Builder#build} gives the spec
   * @throws BuildException if a statement fails a check
   */
  public static Builder builder(Runnable body) {
    return run(null, body, Site.caller());
  }

  /**
   * Builds a reader spec that starts with {@code format name}.
   *
   * @param format the format name
   * @param body the statements
   * @return the builder, whose {@link Builder#build} gives the spec
   * @throws BuildException if a statement fails a check
   */
  public static Builder builder(String format, Runnable body) {
    return run(format, body, Site.caller());
  }

  /**
   * Builds and parses a reader spec.
   *
   * @param body the statements
   * @return the spec
   * @throws BuildException if a check fails, or the language rejects the text
   */
  public static Spec spec(Runnable body) {
    return run(null, body, Site.caller()).build();
  }

  /**
   * Builds and parses a reader spec that starts with {@code format name}.
   *
   * @param format the format name
   * @param body the statements
   * @return the spec
   * @throws BuildException if a check fails, or the language rejects the text
   */
  public static Spec spec(String format, Runnable body) {
    return run(format, body, Site.caller()).build();
  }

  private static Builder run(@Nullable String format, Runnable body, StackTraceElement site) {
    State state = new State();
    Block root = state.root();
    if (format != null) {
      state.push(root, new Stmt.Format(format), site);
    }
    ScopedValue.where(OPEN, root).run(body);
    return new Builder(state);
  }

  // Blocks and checks

  private static Block open(String statement) {
    if (!OPEN.isBound()) {
      throw new IllegalStateException(
          "`"
              + statement
              + "` belongs in the body of builder() or spec(), or of a block inside it");
    }
    return OPEN.get();
  }

  /** The block for a statement other than an arm, which a {@code sections} body does not take. */
  private static Block statementBlock(String statement, StackTraceElement site) {
    Block block = open(statement);
    if (block.kind() == BlockKind.SECTIONS) {
      block
          .state()
          .fail(
              Kind.SCOPE,
              "a `sections` body takes only arms, not `" + statement + "`; use `arm`",
              site);
    }
    return block;
  }

  private static Block rootBlock(String statement, StackTraceElement site) {
    Block block = open(statement);
    if (block.kind() != BlockKind.ROOT) {
      block
          .state()
          .fail(Kind.SCOPE, "`" + statement + "` belongs at the top level of a spec", site);
    }
    return block;
  }

  private static void push(Block block, Stmt stmt, StackTraceElement site) {
    block.state().push(block, stmt, site);
  }

  /** Push a block statement and run {@code body} with its block open. */
  private static void block(
      Block parent,
      Stmt opener,
      BlockKind kind,
      int depth,
      boolean required,
      Runnable body,
      StackTraceElement site) {
    State state = parent.state();
    int index = state.push(parent, opener, site);
    Block inner = state.openBlock(kind, depth, required);
    try {
      ScopedValue.where(OPEN, inner).run(body);
    } finally {
      state.closeBlock(parent, index, site);
    }
  }

  private static void continueIf(Block block, String form, StackTraceElement site) {
    State state = block.state();
    State.ClosedBlock closed = state.closed;
    if (closed == null
        || closed.close() != state.stmts.size() - 1
        || closed.parent() != block.id()
        || !(state.stmts.get(closed.opener()) instanceof Stmt.If
            || state.stmts.get(closed.opener()) instanceof Stmt.ElseIf)) {
      state.fail(
          Kind.SCOPE,
          "`"
              + form
              + "` must directly follow the block of an `ifThen` or `elseIf` in the same"
              + " block",
          site);
    }
  }

  /** Check a name that a factory gives a handle. */
  private static String definitionName(String name, StackTraceElement site) {
    if (name.equals("_")) {
      throw BuildException.check(
          Kind.UNPRINTABLE,
          "`_` names nothing here: use `discard()` to read without storing, or `forward(\"_\","
              + " Str.class)` with `store` for the entry named `_`",
          site);
    }
    return spelledName(name, site);
  }

  /** Check a name that may be {@code _}. */
  private static String spelledName(String name, StackTraceElement site) {
    if (name.startsWith("gecode.")) {
      throw BuildException.check(
          Kind.UNPRINTABLE,
          "a definition cannot be named `" + name + "`: the prefix `gecode.` is reserved",
          site);
    }
    if (!Printer.isIdentifier(name)) {
      throw BuildException.check(Kind.UNPRINTABLE, "`" + name + "` is not an Iris name", site);
    }
    return name;
  }

  /** Resolve a definition's target, declaring a declared handle. */
  private static String target(State state, Field<?> target, StackTraceElement site) {
    Handle handle = Exprs.handle(target);
    state.use(handle, site);
    return handle.name();
  }

  private static Stmt.@Nullable Definition store(
      State state, @Nullable Field<?> target, StackTraceElement site) {
    return target == null ? null : new Stmt.Definition(target(state, target, site), Tag.STR, 0);
  }

  private static List<Op> ops(Expr expr) {
    return Exprs.node(expr).postfix();
  }

  private static List<String> strings(String first, String... more) {
    List<String> all = new ArrayList<>(more.length + 1);
    all.add(first);
    all.addAll(List.of(more));
    return List.copyOf(all);
  }

  // Handles

  private static <V> IntField<V> intField(String name, int levels, boolean valueView) {
    return new IntField<>(
        Handle.declared(definitionName(name, Site.caller()), Tag.INT, levels, valueView));
  }

  private static <V> FloatField<V> floatField(String name, int levels, boolean valueView) {
    return new FloatField<>(
        Handle.declared(definitionName(name, Site.caller()), Tag.FLOAT, levels, valueView));
  }

  private static <V> NumField<V> numField(String name, int levels, boolean valueView) {
    return new NumField<>(
        Handle.declared(definitionName(name, Site.caller()), Tag.NUM, levels, valueView));
  }

  private static <V> StringField<V> stringField(String name, int levels, boolean valueView) {
    return new StringField<>(
        Handle.declared(definitionName(name, Site.caller()), Tag.STR, levels, valueView));
  }

  private static int depth(int depth) {
    if (depth < 0) {
      throw new IllegalArgumentException("a depth is at least 0, not " + depth);
    }
    return depth;
  }

  /**
   * {@return the handle of a single integer}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static IntField<Long> int0(String name) {
    return intField(name, 0, false);
  }

  /**
   * {@return the handle of an array of integers}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static IntField<long[]> int1(String name) {
    return intField(name, 1, false);
  }

  /**
   * {@return the handle of a two-dimensional array of integers}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static IntField<List<long[]>> int2(String name) {
    return intField(name, 2, false);
  }

  /**
   * {@return the handle of a three-dimensional array of integers}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static IntField<List<List<long[]>>> int3(String name) {
    return intField(name, 3, false);
  }

  /**
   * {@return the handle of integers at any depth, read as a {@link Value}}
   *
   * @param name the entry name
   * @param depth the depth
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static IntField<Value> intN(String name, int depth) {
    return intField(name, depth(depth), true);
  }

  /**
   * {@return the handle of a single float}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static FloatField<Double> float0(String name) {
    return floatField(name, 0, false);
  }

  /**
   * {@return the handle of an array of floats}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static FloatField<double[]> float1(String name) {
    return floatField(name, 1, false);
  }

  /**
   * {@return the handle of a two-dimensional array of floats}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static FloatField<List<double[]>> float2(String name) {
    return floatField(name, 2, false);
  }

  /**
   * {@return the handle of a three-dimensional array of floats}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static FloatField<List<List<double[]>>> float3(String name) {
    return floatField(name, 3, false);
  }

  /**
   * {@return the handle of floats at any depth, read as a {@link Value}}
   *
   * @param name the entry name
   * @param depth the depth
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static FloatField<Value> floatN(String name, int depth) {
    return floatField(name, depth(depth), true);
  }

  /**
   * {@return the handle of a single {@code num}, a {@code Long} or a {@code Double} as the data
   * decides}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static NumField<Number> num0(String name) {
    return numField(name, 0, false);
  }

  /**
   * {@return the handle of an array of {@code num}s}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static NumField<List<Number>> num1(String name) {
    return numField(name, 1, false);
  }

  /**
   * {@return the handle of a two-dimensional array of {@code num}s}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static NumField<List<List<Number>>> num2(String name) {
    return numField(name, 2, false);
  }

  /**
   * {@return the handle of a three-dimensional array of {@code num}s}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static NumField<List<List<List<Number>>>> num3(String name) {
    return numField(name, 3, false);
  }

  /**
   * {@return the handle of {@code num}s at any depth, read as a {@link Value}}
   *
   * @param name the entry name
   * @param depth the depth
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static NumField<Value> numN(String name, int depth) {
    return numField(name, depth(depth), true);
  }

  /**
   * {@return the handle of a single string}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static StringField<String> string0(String name) {
    return stringField(name, 0, false);
  }

  /**
   * {@return the handle of an array of strings}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static StringField<List<String>> string1(String name) {
    return stringField(name, 1, false);
  }

  /**
   * {@return the handle of a two-dimensional array of strings}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static StringField<List<List<String>>> string2(String name) {
    return stringField(name, 2, false);
  }

  /**
   * {@return the handle of a three-dimensional array of strings}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static StringField<List<List<List<String>>>> string3(String name) {
    return stringField(name, 3, false);
  }

  /**
   * {@return the handle of strings at any depth, read as a {@link Value}}
   *
   * @param name the entry name
   * @param depth the depth
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static StringField<Value> stringN(String name, int depth) {
    return stringField(name, depth(depth), true);
  }

  /**
   * {@return the handle of an entry whose value type and depth are open, read as a {@link Value}
   * that is null when the entry is absent}
   *
   * @param name the entry name
   * @throws BuildException if {@code name} is not an Iris name
   */
  public static DynField dynamic(String name) {
    String checked = definitionName(name, Site.caller());
    return new DynField(new Handle(checked, Tag.DYN, null, false, Handle.Origin.DECLARED, 0, true));
  }

  /**
   * Declares a name in the open builder and returns its handle, of the kind {@code kind} names:
   * {@code Int.class}, {@code Real.class}, {@code Num.class}, {@code Str.class} or {@code
   * Dyn.class}. The handle is a required value at depth 0, or open for {@code Dyn.class}, and
   * belongs to this builder.
   *
   * <p>{@code forward("_", Str.class)} is the handle of the entry that {@code store _} keeps.
   * {@link Builder#build} reports a handle that nothing defines, and a definition that does not
   * have its type, unless an earlier one always precedes it.
   *
   * @param <H> the handle class
   * @param name the entry name, which may be {@code _}
   * @param kind the kind of handle
   * @return the handle
   * @throws BuildException if {@code name} is not an Iris name or is already declared
   */
  @SuppressWarnings("unchecked")
  public static <H extends Field<?>> H forward(String name, Class<? extends HandleKind<H>> kind) {
    StackTraceElement site = Site.caller();
    State state = open("forward").state();
    String checked = spelledName(name, site);
    Class<?> token = kind;
    Tag tag;
    if (token == Int.class) {
      tag = Tag.INT;
    } else if (token == Real.class) {
      tag = Tag.FLOAT;
    } else if (token == Num.class) {
      tag = Tag.NUM;
    } else if (token == Str.class) {
      tag = Tag.STR;
    } else {
      tag = Tag.DYN;
    }
    boolean open = tag == Tag.DYN;
    Handle handle =
        new Handle(checked, tag, open ? null : 0, !open, Handle.Origin.FORWARD, state.id, open);
    state.declare(checked, new State.Forward(handle.shape(), null), site);
    return (H) Exprs.field(handle);
  }

  /**
   * Declares a name that is deliberately left undefined, as in {@code check !defined(name)}, and
   * returns its handle. Defining it fails {@link Builder#build}.
   *
   * @param name the entry name
   * @return the handle
   * @throws BuildException if {@code name} is not an Iris name or is already declared
   */
  public static DynField undefined(String name) {
    StackTraceElement site = Site.caller();
    State state = open("undefined").state();
    String checked = spelledName(name, site);
    state.declare(checked, new State.Undefined(), site);
    return new DynField(
        new Handle(checked, Tag.DYN, null, false, Handle.Origin.UNDEFINED, state.id, true));
  }

  /**
   * Declares the name of {@code handle}, a handle from a factory such as {@link #int0}, in the open
   * builder, and returns the handle. Its first use would declare the name too, but a use accepts a
   * name that an equal handle declared before; this fails when anything has declared the name.
   *
   * <p>A builder whose definitions take a name rather than a handle, such as the Kotlin builder,
   * declares each name with it, so that a helper called twice with one name is an error rather than
   * a redefinition. Redefinition goes through the handle.
   *
   * @param <H> the handle class
   * @param handle the handle to declare
   * @return {@code handle}
   * @throws BuildException if the name is already declared in this builder
   * @throws IllegalArgumentException if {@code handle} does not come from a factory
   */
  public static <H extends Field<?>> H declare(H handle) {
    StackTraceElement site = Site.caller();
    State state = open("declare").state();
    Handle declared = Exprs.handle(handle);
    if (declared.origin() != Handle.Origin.DECLARED) {
      throw new IllegalArgumentException(
          "the handle `"
              + declared.name()
              + "` comes from a builder, not a factory such as `int0`, so it is already declared");
    }
    state.declare(declared.name(), new State.Forward(declared.shape(), declared), site);
    return handle;
  }

  /** {@return a record name, printed with names the builder chooses} */
  public static RecordName recordName() {
    return new RecordName(null, null);
  }

  /**
   * {@return a record name, printed as {@code each} for the current record and {@code list} for all
   * of them}
   *
   * @param each the {@code each} name, which may be {@code _}
   * @param list the {@code as} name, which may be {@code _}
   * @throws BuildException if a name is not an Iris name
   */
  public static RecordName recordName(String each, String list) {
    StackTraceElement site = Site.caller();
    return new RecordName(spelledName(each, site), spelledName(list, site));
  }

  // Settings

  /**
   * {@code format name}, a name of this format.
   *
   * @param name the format name
   */
  public static void format(String name) {
    StackTraceElement site = Site.caller();
    push(rootBlock("format", site), new Stmt.Format(name), site);
  }

  /**
   * {@code comment "text"...}.
   *
   * @param marker a comment marker
   * @param more more comment markers
   */
  public static void comment(String marker, String... more) {
    StackTraceElement site = Site.caller();
    push(rootBlock("comment", site), new Stmt.Strings("comment", strings(marker, more)), site);
  }

  /**
   * {@code stop "text"...}.
   *
   * @param marker a stop marker
   * @param more more stop markers
   */
  public static void stop(String marker, String... more) {
    StackTraceElement site = Site.caller();
    push(rootBlock("stop", site), new Stmt.Strings("stop", strings(marker, more)), site);
  }

  /**
   * {@code tokens "characters"}.
   *
   * @param characters the characters that are tokens of their own
   */
  public static void tokens(String characters) {
    StackTraceElement site = Site.caller();
    push(rootBlock("tokens", site), new Stmt.Strings("tokens", List.of(characters)), site);
  }

  /**
   * {@code separators "characters"}.
   *
   * @param characters the characters that separate tokens
   */
  public static void separators(String characters) {
    StackTraceElement site = Site.caller();
    push(rootBlock("separators", site), new Stmt.Strings("separators", List.of(characters)), site);
  }

  /** {@code ignore trailing}. */
  public static void ignoreTrailing() {
    StackTraceElement site = Site.caller();
    push(rootBlock("ignoreTrailing", site), new Stmt.IgnoreTrailing(), site);
  }

  /**
   * {@code instances { ... }}.
   *
   * @param body the statements of one instance
   */
  public static void instances(Runnable body) {
    StackTraceElement site = Site.caller();
    block(
        rootBlock("instances", site),
        new Stmt.Instances(),
        BlockKind.INSTANCES,
        0,
        true,
        body,
        site);
  }

  // Definitions

  /**
   * {@code x: type}, reading a value into the entry of {@code target}.
   *
   * @param target the handle of the entry
   * @param type the type, with any dimensions and options
   * @return the handle of the name as this definition leaves it
   */
  public static DynField read(Field<?> target, Type<?> type) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("read", site);
    State state = block.state();
    String name = target(state, target, site);
    Declared declared = type.declared;
    push(
        block,
        new Stmt.Declaration(
            new Stmt.Definition(name, type.tag(), declared.dimensions().size()), declared),
        site);
    return state.reference(name);
  }

  /**
   * {@code _: type}, reading a value without storing it.
   *
   * @param target {@link #discard()}
   * @param type the type, with any dimensions and options
   */
  public static void read(Discard target, Type<?> type) {
    StackTraceElement site = Site.caller();
    Declared declared = type.declared;
    push(
        statementBlock("read", site),
        new Stmt.Declaration(
            new Stmt.Definition(null, type.tag(), declared.dimensions().size()), declared),
        site);
  }

  /**
   * {@code list x: type until end}.
   *
   * @param target the handle of the entry
   * @param reading the type and end, from {@code type.until(end)}
   * @return the handle of the name as this definition leaves it
   */
  public static DynField list(Field<?> target, ListReading reading) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("list", site);
    State state = block.state();
    String name = target(state, target, site);
    push(
        block,
        new Stmt.ListStmt(new Stmt.Definition(name, reading.tag, 1), reading.declared, reading.end),
        site);
    return state.reference(name);
  }

  /**
   * {@code list _: type until end}.
   *
   * @param target {@link #discard()}
   * @param reading the type and end, from {@code type.until(end)}
   */
  public static void list(Discard target, ListReading reading) {
    StackTraceElement site = Site.caller();
    push(
        statementBlock("list", site),
        new Stmt.ListStmt(new Stmt.Definition(null, reading.tag, 1), reading.declared, reading.end),
        site);
  }

  /**
   * {@code let x = value}.
   *
   * @param target the handle of the entry
   * @param value the value
   * @return the handle of the name as this definition leaves it
   */
  public static DynField let(Field<?> target, Expr value) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("let", site);
    State state = block.state();
    String name = target(state, target, site);
    Integer levels = Exprs.letLevels(value, block.depth());
    push(
        block, new Stmt.Let(new Stmt.Definition(name, Exprs.tag(value), levels), ops(value)), site);
    return state.reference(name);
  }

  /**
   * {@code let _ = value}, which evaluates {@code value} without storing it.
   *
   * @param target {@link #discard()}
   * @param value the value
   */
  public static void let(Discard target, Expr value) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("let", site);
    Integer levels = Exprs.letLevels(value, block.depth());
    push(
        block, new Stmt.Let(new Stmt.Definition(null, Exprs.tag(value), levels), ops(value)), site);
  }

  /**
   * {@code param p}, whose value and type the caller decides.
   *
   * @param target the handle of the parameter
   * @return the handle of the name as this definition leaves it
   */
  public static DynField param(Field<?> target) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("param", site);
    State state = block.state();
    String name = target(state, target, site);
    push(block, new Stmt.Param(new Stmt.Definition(name, Tag.DYN, null), null), site);
    return state.reference(name);
  }

  /**
   * {@code param p = default}.
   *
   * @param target the handle of the parameter
   * @param defaultValue the value when the caller gives none
   * @return the handle of the name as this definition leaves it
   */
  public static DynField param(Field<?> target, Expr defaultValue) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("param", site);
    State state = block.state();
    String name = target(state, target, site);
    Tag tag = Exprs.tag(defaultValue);
    boolean scalar = tag != Tag.DYN && !Exprs.open(defaultValue);
    push(
        block,
        new Stmt.Param(new Stmt.Definition(name, tag, scalar ? 0 : null), ops(defaultValue)),
        site);
    return state.reference(name);
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if {@code value} has another depth than the handle
   */
  public static void set(IntField<?> handle, IntExpr value) {
    setValue(handle, value, Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if the handle is not a single value
   */
  public static void set(IntField<?> handle, long value) {
    setValue(handle, lit(value), Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if {@code value} has another depth than the handle
   */
  public static void set(FloatField<?> handle, FloatExpr value) {
    setValue(handle, value, Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if the handle is not a single value
   */
  public static void set(FloatField<?> handle, double value) {
    setValue(handle, lit(value), Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if {@code value} has another depth than the handle
   */
  public static void set(NumField<?> handle, NumExpr value) {
    setValue(handle, value, Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if {@code value} has another depth than the handle
   */
  public static void set(StringField<?> handle, StringExpr value) {
    setValue(handle, value, Site.caller());
  }

  /**
   * {@code let x = value} through a handle, keeping its type and depth.
   *
   * @param handle the handle
   * @param value the value
   * @throws BuildException if the handle is not a single value
   */
  public static void set(StringField<?> handle, String value) {
    setValue(handle, lit(value), Site.caller());
  }

  /**
   * {@code let x = value} through a handle whose type is open.
   *
   * @param handle the handle
   * @param value the value
   */
  public static void set(DynField handle, Expr value) {
    setValue(handle, value, Site.caller());
  }

  private static void setValue(Field<?> field, Expr value, StackTraceElement site) {
    Block block = statementBlock("set", site);
    State state = block.state();
    Handle handle = Exprs.handle(field);
    String name = target(state, field, site);
    Integer own = Exprs.letLevels(value, block.depth());
    Integer total = own == null ? null : block.depth() + own;
    if (handle.levels() != null && total != null && !handle.levels().equals(total)) {
      state.fail(
          Kind.BINDING,
          "`set` keeps the depth "
              + handle.levels()
              + " of `"
              + name
              + "`, but this value has depth "
              + total
              + "; to change the shape, redefine the name with `let(handle, value)`",
          site);
    }
    push(block, new Stmt.Let(new Stmt.Definition(name, handle.tag(), own), ops(value)), site);
  }

  /**
   * {@code type name = type}, usable wherever a type is. The type it returns belongs to this
   * declaration: {@link Builder#build} reports a use where another declaration of the name is
   * visible.
   *
   * @param name the type name
   * @param type the type, without dimensions
   * @return the named type
   */
  public static IntType defineType(String name, IntType type) {
    return new IntType(defineType(name, type, Site.caller()));
  }

  /**
   * {@code type name = type}.
   *
   * @param name the type name
   * @param type the type, without dimensions
   * @return the named type
   */
  public static FloatType defineType(String name, FloatType type) {
    return new FloatType(defineType(name, type, Site.caller()));
  }

  /**
   * {@code type name = type}.
   *
   * @param name the type name
   * @param type the type, without dimensions
   * @return the named type
   */
  public static NumType defineType(String name, NumType type) {
    return new NumType(defineType(name, type, Site.caller()));
  }

  /**
   * {@code type name = type}.
   *
   * @param name the type name
   * @param type the type, without dimensions
   * @return the named type
   */
  public static StringType defineType(String name, StringType type) {
    return new StringType(defineType(name, type, Site.caller()));
  }

  private static Declared defineType(String name, Type<?> type, StackTraceElement site) {
    Block block = statementBlock("defineType", site);
    State state = block.state();
    Declared declared = type.scalar("a named type");
    String checked = definitionName(name, site);
    state.types.add(checked);
    int index = state.push(block, new Stmt.TypeDefinition(checked, declared), site);
    return new Declared(new Declared.NamedSpec(checked, state.id, index));
  }

  // Statements

  /**
   * {@code check condition}.
   *
   * @param condition the condition
   */
  public static void check(Expr condition) {
    StackTraceElement site = Site.caller();
    push(statementBlock("check", site), new Stmt.Check(ops(condition), null), site);
  }

  /**
   * {@code check condition "message"}.
   *
   * @param condition the condition
   * @param message the message when it fails
   */
  public static void check(Expr condition, String message) {
    StackTraceElement site = Site.caller();
    push(statementBlock("check", site), new Stmt.Check(ops(condition), message), site);
  }

  /**
   * {@code expect "text"...}.
   *
   * @param word an expected word
   * @param more more expected words
   */
  public static void expect(String word, String... more) {
    StackTraceElement site = Site.caller();
    push(statementBlock("expect", site), new Stmt.Strings("expect", strings(word, more)), site);
  }

  /**
   * {@code line "pattern"}.
   *
   * @param parts the pattern: text, handles, parts and patterns, as {@link #pattern} takes
   */
  public static void line(Object... parts) {
    StackTraceElement site = Site.caller();
    push(statementBlock("line", site), new Stmt.Line("line", Pattern.of(parts).pieces), site);
  }

  /**
   * {@code find "pattern"}.
   *
   * @param parts the pattern: text, handles, parts and patterns, as {@link #pattern} takes
   */
  public static void find(Object... parts) {
    StackTraceElement site = Site.caller();
    push(statementBlock("find", site), new Stmt.Line("find", Pattern.of(parts).pieces), site);
  }

  /** {@code skip line}. */
  public static void skipLine() {
    StackTraceElement site = Site.caller();
    push(statementBlock("skipLine", site), new Stmt.SkipLine(), site);
  }

  /**
   * {@code skip count lines}.
   *
   * @param count the number of lines
   */
  public static void skipLines(Expr count) {
    StackTraceElement site = Site.caller();
    push(statementBlock("skipLines", site), new Stmt.SkipLines(ops(count)), site);
  }

  /**
   * {@code skip count lines}.
   *
   * @param count the number of lines
   */
  public static void skipLines(long count) {
    StackTraceElement site = Site.caller();
    push(statementBlock("skipLines", site), new Stmt.SkipLines(ops(lit(count))), site);
  }

  /**
   * {@code index position}.
   *
   * @param position the position
   */
  public static void index(Expr position) {
    StackTraceElement site = Site.caller();
    push(statementBlock("index", site), new Stmt.Index(ops(position)), site);
  }

  /**
   * {@code index position}.
   *
   * @param position the position
   */
  public static void index(long position) {
    StackTraceElement site = Site.caller();
    push(statementBlock("index", site), new Stmt.Index(ops(lit(position))), site);
  }

  // Repeats

  /**
   * {@code repeat count { ... }}.
   *
   * @param count the number of times
   * @param body the statements of one time
   */
  public static void repeat(Expr count, Runnable body) {
    repeat(new Stmt.Times(ops(count)), null, body, Site.caller());
  }

  /**
   * {@code repeat count { ... }}.
   *
   * @param count the number of times
   * @param body the statements of one time
   */
  public static void repeat(long count, Runnable body) {
    repeat(new Stmt.Times(ops(lit(count))), null, body, Site.caller());
  }

  /**
   * {@code repeat count { ... }}, whose records {@code record} names.
   *
   * @param count the number of times
   * @param record the record name, which no other repeat of this builder binds
   * @param body the statements of one time
   * @throws BuildException if another repeat bound {@code record}
   */
  public static void repeat(Expr count, RecordName record, Runnable body) {
    repeat(new Stmt.Times(ops(count)), record, body, Site.caller());
  }

  /**
   * {@code repeat count { ... }}, whose records {@code record} names.
   *
   * @param count the number of times
   * @param record the record name, which no other repeat of this builder binds
   * @param body the statements of one time
   * @throws BuildException if another repeat bound {@code record}
   */
  public static void repeat(long count, RecordName record, Runnable body) {
    repeat(new Stmt.Times(ops(lit(count))), record, body, Site.caller());
  }

  /**
   * {@code repeat until eof { ... }}.
   *
   * @param body the statements of one time
   */
  public static void repeatUntilEof(Runnable body) {
    repeat(new Stmt.UntilEof(), null, body, Site.caller());
  }

  /**
   * {@code repeat until eof { ... }}, whose records {@code record} names.
   *
   * @param record the record name, which no other repeat of this builder binds
   * @param body the statements of one time
   * @throws BuildException if another repeat bound {@code record}
   */
  public static void repeatUntilEof(RecordName record, Runnable body) {
    repeat(new Stmt.UntilEof(), record, body, Site.caller());
  }

  /**
   * {@code repeat until "text"... { ... }}.
   *
   * @param until the terminators, from {@link #until}, with any {@code store}
   * @param body the statements of one time
   */
  public static void repeatUntil(Until until, Runnable body) {
    StackTraceElement site = Site.caller();
    repeat(terminated(until, site), null, body, site);
  }

  /**
   * {@code repeat until "text"... { ... }}, whose records {@code record} names.
   *
   * @param until the terminators, from {@link #until}, with any {@code store}
   * @param record the record name, which no other repeat of this builder binds
   * @param body the statements of one time
   * @throws BuildException if another repeat bound {@code record}
   */
  public static void repeatUntil(Until until, RecordName record, Runnable body) {
    StackTraceElement site = Site.caller();
    repeat(terminated(until, site), record, body, site);
  }

  private static Stmt.Terminated terminated(Until until, StackTraceElement site) {
    State state = statementBlock("repeatUntil", site).state();
    return new Stmt.Terminated(until.terminators, store(state, until.store, site));
  }

  private static void repeat(
      Stmt.RepeatCount count, @Nullable RecordName record, Runnable body, StackTraceElement site) {
    Block block = statementBlock("repeat", site);
    State state = block.state();
    int index = record == null ? state.anonymousRecord(site) : state.bind(record, site);
    block(
        block,
        new Stmt.Repeat(count, index),
        BlockKind.REPEAT,
        block.depth() + 1,
        block.required(),
        body,
        site);
  }

  /**
   * {@return the terminators {@code until "text"...} of a repeat or sections}
   *
   * @param terminator a terminator
   * @param more more terminators
   */
  public static Until until(String terminator, String... more) {
    return new Until(strings(terminator, more), null);
  }

  /** {@return the {@code skip other} clause of sections} */
  public static SectionClause skipOther() {
    return SkipOther.INSTANCE;
  }

  // Sections

  /**
   * {@code sections { ... }}, whose body holds only arms.
   *
   * @param body the arms
   */
  public static void sections(Runnable body) {
    sections(List.of(), body, Site.caller());
  }

  /**
   * {@code sections clause { ... }}.
   *
   * @param clause the clause
   * @param body the arms
   */
  public static void sections(SectionClause clause, Runnable body) {
    sections(List.of(clause), body, Site.caller());
  }

  /**
   * {@code sections clause clause { ... }}.
   *
   * @param first the first clause
   * @param second the second clause
   * @param body the arms
   */
  public static void sections(SectionClause first, SectionClause second, Runnable body) {
    sections(List.of(first, second), body, Site.caller());
  }

  /**
   * {@code sections clause clause clause { ... }}.
   *
   * @param first the first clause
   * @param second the second clause
   * @param third the third clause
   * @param body the arms
   */
  public static void sections(
      SectionClause first, SectionClause second, SectionClause third, Runnable body) {
    sections(List.of(first, second, third), body, Site.caller());
  }

  /**
   * {@code sections clause... { ... }}, with any number of clauses.
   *
   * @param clauses the clauses, in order
   * @param body the arms
   */
  public static void sections(List<? extends SectionClause> clauses, Runnable body) {
    sections(clauses, body, Site.caller());
  }

  private static void sections(
      List<? extends SectionClause> clauses, Runnable body, StackTraceElement site) {
    Block block = statementBlock("sections", site);
    State state = block.state();
    List<Stmt.Clause> resolved = new ArrayList<>(clauses.size());
    for (SectionClause clause : clauses) {
      resolved.add(
          switch (clause) {
            case Until until ->
                new Stmt.Terminated(until.terminators, store(state, until.store, site));
            case SkipOther skip -> new Stmt.SkipOtherClause();
          });
    }
    block(
        block,
        new Stmt.Sections(List.copyOf(resolved)),
        BlockKind.SECTIONS,
        block.depth(),
        block.required(),
        body,
        site);
  }

  /** {@return the {@code optional} option of an arm} */
  public static ArmOption optional() {
    return ArmOption.OPTIONAL;
  }

  /** {@return the {@code last} option of an arm} */
  public static ArmOption last() {
    return ArmOption.LAST;
  }

  /**
   * An arm of the enclosing sections, with no body.
   *
   * @param text the pattern, as text
   * @param options {@code optional()} and {@code last()}
   */
  public static void arm(String text, ArmOption... options) {
    arm(Pattern.of(text), List.of(options), null, Site.caller());
  }

  /**
   * An arm of the enclosing sections, with no body.
   *
   * @param pattern the pattern
   * @param options {@code optional()} and {@code last()}
   */
  public static void arm(Pattern pattern, ArmOption... options) {
    arm(pattern, List.of(options), null, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param text the pattern, as text
   * @param body the statements after the arm's line
   */
  public static void arm(String text, Runnable body) {
    arm(Pattern.of(text), List.of(), body, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param pattern the pattern
   * @param body the statements after the arm's line
   */
  public static void arm(Pattern pattern, Runnable body) {
    arm(pattern, List.of(), body, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param text the pattern, as text
   * @param option {@code optional()} or {@code last()}
   * @param body the statements after the arm's line
   */
  public static void arm(String text, ArmOption option, Runnable body) {
    arm(Pattern.of(text), List.of(option), body, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param pattern the pattern
   * @param option {@code optional()} or {@code last()}
   * @param body the statements after the arm's line
   */
  public static void arm(Pattern pattern, ArmOption option, Runnable body) {
    arm(pattern, List.of(option), body, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param text the pattern, as text
   * @param first {@code optional()} or {@code last()}
   * @param second {@code optional()} or {@code last()}
   * @param body the statements after the arm's line
   */
  public static void arm(String text, ArmOption first, ArmOption second, Runnable body) {
    arm(Pattern.of(text), List.of(first, second), body, Site.caller());
  }

  /**
   * An arm of the enclosing sections.
   *
   * @param pattern the pattern
   * @param first {@code optional()} or {@code last()}
   * @param second {@code optional()} or {@code last()}
   * @param body the statements after the arm's line
   */
  public static void arm(Pattern pattern, ArmOption first, ArmOption second, Runnable body) {
    arm(pattern, List.of(first, second), body, Site.caller());
  }

  private static void arm(
      Pattern pattern, List<ArmOption> options, @Nullable Runnable body, StackTraceElement site) {
    Block block = open("arm");
    if (block.kind() != BlockKind.SECTIONS) {
      block.state().fail(Kind.SCOPE, "an arm belongs directly in the body of `sections`", site);
    }
    boolean optional = options.contains(ArmOption.OPTIONAL);
    boolean last = options.contains(ArmOption.LAST);
    Stmt.Arm arm = new Stmt.Arm(pattern.pieces, optional, last, body != null);
    if (body == null) {
      push(block, arm, site);
    } else {
      block(block, arm, BlockKind.ARM, block.depth(), block.required() && !optional, body, site);
    }
  }

  // Branches

  /**
   * {@code if condition { ... }}. What its block defines may be absent.
   *
   * @param condition the condition
   * @param body the statements when it holds
   */
  public static void ifThen(Expr condition, Runnable body) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("ifThen", site);
    block(block, new Stmt.If(ops(condition)), BlockKind.BRANCH, block.depth(), false, body, site);
  }

  /**
   * {@code else if condition { ... }}, directly after an {@code ifThen} or {@code elseIf} block.
   *
   * @param condition the condition
   * @param body the statements when it holds
   * @throws BuildException if no {@code ifThen} or {@code elseIf} block comes just before
   */
  public static void elseIf(Expr condition, Runnable body) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("elseIf", site);
    continueIf(block, "elseIf", site);
    block(
        block, new Stmt.ElseIf(ops(condition)), BlockKind.BRANCH, block.depth(), false, body, site);
  }

  /**
   * {@code else { ... }}, directly after an {@code ifThen} or {@code elseIf} block.
   *
   * @param body the statements when no condition holds
   * @throws BuildException if no {@code ifThen} or {@code elseIf} block comes just before
   */
  public static void otherwise(Runnable body) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("otherwise", site);
    continueIf(block, "otherwise", site);
    block(block, new Stmt.Else(), BlockKind.BRANCH, block.depth(), false, body, site);
  }

  /**
   * {@code else if:type}: the else branch is a declaration of the name {@code if}, which the target
   * must have.
   *
   * @param target the handle of the entry named {@code if}
   * @param type the type
   * @return the handle of the name as this definition leaves it
   * @throws BuildException if no {@code ifThen} or {@code elseIf} block comes just before, or the
   *     target has another name
   */
  public static DynField elseReadIf(Field<?> target, Type<?> type) {
    StackTraceElement site = Site.caller();
    Block block = statementBlock("elseReadIf", site);
    State state = block.state();
    continueIf(block, "elseReadIf", site);
    if (!target.name().equals("if")) {
      state.fail(
          Kind.UNPRINTABLE,
          "`elseReadIf` declares the name `if`, which its target must have",
          site);
    }
    String name = target(state, target, site);
    Declared declared = type.declared;
    push(
        block,
        new Stmt.ElseDeclaration(
            new Stmt.Definition(name, type.tag(), declared.dimensions().size()), declared),
        site);
    return state.reference(name);
  }

  // Types

  /** {@return {@code int}} */
  public static IntType intType() {
    return new IntType(new Declared(new Declared.BaseSpec("int")));
  }

  /** {@return {@code float}} */
  public static FloatType floatType() {
    return new FloatType(new Declared(new Declared.BaseSpec("float")));
  }

  /** {@return {@code num}, an integer or a float as the data decides} */
  public static NumType numType() {
    return new NumType(new Declared(new Declared.BaseSpec("num")));
  }

  /** {@return {@code word}} */
  public static StringType word() {
    return new StringType(new Declared(new Declared.BaseSpec("word")));
  }

  /** {@return {@code rest}, the rest of the line} */
  public static StringType rest() {
    return new StringType(new Declared(new Declared.BaseSpec("rest")));
  }

  /**
   * {@return {@code alphabet "symbols"}}
   *
   * @param symbols the symbols, whose indices are the values
   */
  public static Alphabet alphabet(String symbols) {
    return new Alphabet(new Declared(new Declared.AlphabetSpec(symbols, List.of())));
  }

  /**
   * {@return {@code lo..}, which {@link Range#to} and {@link Range#below} bound}
   *
   * @param lo the lower bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public static Range range(NumericExpr lo) {
    return new Range(Exprs.bound(lo));
  }

  /**
   * {@return {@code lo..}, which {@link Range#to} and {@link Range#below} bound}
   *
   * @param lo the lower bound
   */
  public static Range range(long lo) {
    return new Range(Exprs.literal(lo));
  }

  /**
   * {@return {@code lo..}, the integers from {@code lo} up}
   *
   * @param lo the lower bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public static IntType atLeast(NumericExpr lo) {
    return range(lo);
  }

  /**
   * {@return {@code lo..}, the integers from {@code lo} up}
   *
   * @param lo the lower bound
   */
  public static IntType atLeast(long lo) {
    return range(lo);
  }

  /**
   * {@return {@code ..hi}, the integers up to {@code hi}}
   *
   * @param hi the upper bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public static IntType atMost(NumericExpr hi) {
    return upTo(Exprs.bound(hi), true);
  }

  /**
   * {@return {@code ..hi}, the integers up to {@code hi}}
   *
   * @param hi the upper bound
   */
  public static IntType atMost(long hi) {
    return upTo(Exprs.literal(hi), true);
  }

  /**
   * {@return {@code ..<hi}, the integers below {@code hi}}
   *
   * @param hi the upper bound
   * @throws IllegalArgumentException if the bound is a float
   */
  public static IntType below(NumericExpr hi) {
    return upTo(Exprs.bound(hi), false);
  }

  /**
   * {@return {@code ..<hi}, the integers below {@code hi}}
   *
   * @param hi the upper bound
   */
  public static IntType below(long hi) {
    return upTo(Exprs.literal(hi), false);
  }

  private static IntType upTo(Node upper, boolean inclusive) {
    return new IntType(new Declared(new Declared.RangeSpec(null, upper.postfix(), inclusive)));
  }

  // Patterns

  /**
   * {@return a line pattern of text and parts}
   *
   * @param parts the parts: {@code String} text; a handle, which is {@code {x}} and reads a word; a
   *     {@link Part} such as {@link #capture}; {@link #discard()}, which is {@code {_}}; or another
   *     pattern
   * @throws IllegalArgumentException for a part of another class
   */
  public static Pattern pattern(Object... parts) {
    return Pattern.of(parts);
  }

  /**
   * {@return {@code {x:type}}, which reads {@code type} into the entry of {@code handle}}
   *
   * @param handle the handle of the entry
   * @param type the type, without dimensions
   */
  public static Part capture(Field<?> handle, Type<?> type) {
    return new Part(
        new Piece.Bind(Exprs.handle(handle), type.tag(), type.scalar("a capture's type")));
  }

  /**
   * {@return {@code {$x}}, which matches the current value of {@code handle}'s entry}
   *
   * @param handle the handle of the entry
   */
  public static Part ref(Field<?> handle) {
    return new Part(new Piece.Reference(Exprs.handle(handle)));
  }

  /**
   * {@return {@code {:type}}, which checks the text without storing it}
   *
   * @param type the type, without dimensions
   */
  public static Part validate(Type<?> type) {
    return new Part(new Piece.Validate(type.scalar("a capture's type")));
  }

  /** {@return {@code {}}, which matches a word without storing it} */
  public static Part blank() {
    return new Part(new Piece.Blank());
  }

  /** {@return {@code _}: a target that reads without storing, and {@code {_}} in a pattern} */
  public static Discard discard() {
    return Discard.INSTANCE;
  }

  /**
   * {@return {@code {_:type}}, a discarded capture with a type}
   *
   * @param type the type, without dimensions
   */
  public static Part discard(Type<?> type) {
    return new Part(new Piece.DiscardAs(type.scalar("a capture's type")));
  }

  /** {@return the trailing {@code ...} of a pattern} */
  public static Part restOfLine() {
    return new Part(new Piece.RestOfLine());
  }

  // Expressions

  /**
   * {@return an integer literal}
   *
   * @param value the integer
   */
  public static IntExpr lit(long value) {
    return new Exprs.IntImpl(Exprs.literal(value), false);
  }

  /**
   * {@return a float literal}
   *
   * @param value the float
   */
  public static FloatExpr lit(double value) {
    return new Exprs.FloatImpl(Exprs.literal(value), false);
  }

  /**
   * {@return a string literal}
   *
   * @param value the string
   */
  public static StringExpr lit(String value) {
    return new Exprs.StringImpl(Exprs.literal(value), false);
  }

  private static List<Expr> arguments(Expr first, Expr... more) {
    List<Expr> all = new ArrayList<>(more.length + 1);
    all.add(first);
    all.addAll(List.of(more));
    return all;
  }

  /**
   * {@return {@code sum(a, b, ...)}; of one array, the sum of its elements}
   *
   * @param first the first argument
   * @param more more arguments
   */
  public static IntExpr sum(Expr first, Expr... more) {
    return Exprs.intExpr(Exprs.call("sum", arguments(first, more)));
  }

  /**
   * {@return {@code sum(a, b, ...)}}
   *
   * @param arguments the arguments, at least one
   */
  public static IntExpr sum(List<? extends Expr> arguments) {
    return Exprs.intExpr(Exprs.call("sum", arguments));
  }

  /**
   * {@return {@code min(a, b, ...)}; of one array, its least element}
   *
   * @param first the first argument
   * @param more more arguments
   */
  public static IntExpr min(Expr first, Expr... more) {
    return Exprs.intExpr(Exprs.call("min", arguments(first, more)));
  }

  /**
   * {@return {@code min(a, b, ...)}}
   *
   * @param arguments the arguments, at least one
   */
  public static IntExpr min(List<? extends Expr> arguments) {
    return Exprs.intExpr(Exprs.call("min", arguments));
  }

  /**
   * {@return {@code max(a, b, ...)}; of one array, its greatest element}
   *
   * @param first the first argument
   * @param more more arguments
   */
  public static IntExpr max(Expr first, Expr... more) {
    return Exprs.intExpr(Exprs.call("max", arguments(first, more)));
  }

  /**
   * {@return {@code max(a, b, ...)}}
   *
   * @param arguments the arguments, at least one
   */
  public static IntExpr max(List<? extends Expr> arguments) {
    return Exprs.intExpr(Exprs.call("max", arguments));
  }

  /**
   * {@return {@code distinct(a, b, ...)}; of one array, whether its elements differ}
   *
   * @param first the first argument
   * @param more more arguments
   */
  public static IntExpr distinct(Expr first, Expr... more) {
    return Exprs.intExpr(Exprs.call("distinct", arguments(first, more)));
  }

  /**
   * {@return {@code distinct(a, b, ...)}}
   *
   * @param arguments the arguments, at least one
   */
  public static IntExpr distinct(List<? extends Expr> arguments) {
    return Exprs.intExpr(Exprs.call("distinct", arguments));
  }

  /**
   * {@return {@code sum(x in domain: body)}}
   *
   * @param <E> what the domain gives the body
   * @param domain the domain
   * @param body a function of the comprehension variable
   */
  public static <E> IntExpr sum(Domain<E> domain, Function<? super E, ? extends Expr> body) {
    return Exprs.comprehension("sum", domain, body);
  }

  /**
   * {@return {@code all(x in domain: body)}}
   *
   * @param <E> what the domain gives the body
   * @param domain the domain
   * @param body a function of the comprehension variable
   */
  public static <E> IntExpr all(Domain<E> domain, Function<? super E, ? extends Expr> body) {
    return Exprs.comprehension("all", domain, body);
  }

  /**
   * {@return {@code any(x in domain: body)}}
   *
   * @param <E> what the domain gives the body
   * @param domain the domain
   * @param body a function of the comprehension variable
   */
  public static <E> IntExpr any(Domain<E> domain, Function<? super E, ? extends Expr> body) {
    return Exprs.comprehension("any", domain, body);
  }

  /**
   * {@return {@code count(x in domain: body)}}
   *
   * @param <E> what the domain gives the body
   * @param domain the domain
   * @param body a function of the comprehension variable
   */
  public static <E> IntExpr count(Domain<E> domain, Function<? super E, ? extends Expr> body) {
    return Exprs.comprehension("count", domain, body);
  }

  /**
   * {@return {@code count(domain)}}
   *
   * @param domain the domain
   */
  public static IntExpr count(Domain<?> domain) {
    return Exprs.intExpr(new Node(new Op.CallOp("count", 1), Exprs.domain(domain)));
  }

  /**
   * {@return {@code size(domain)}}
   *
   * @param domain the domain
   */
  public static IntExpr size(Domain<?> domain) {
    return Exprs.intExpr(new Node(new Op.CallOp("size", 1), Exprs.domain(domain)));
  }

  /**
   * {@return {@code defined(name)}}
   *
   * @param name a handle, a lookup or a comprehension variable
   */
  public static IntExpr defined(Expr name) {
    return Exprs.intExpr(new Node(new Op.CallOp("defined", 1), Exprs.node(name)));
  }

  /**
   * {@return {@code defined(records)}, of the record list of {@code record}}
   *
   * @param record the record name
   */
  public static IntExpr defined(RecordName record) {
    return defined(record.list());
  }

  /**
   * {@return {@code defined(records)}}
   *
   * @param records the record list
   */
  public static IntExpr defined(RecordList records) {
    return Exprs.intExpr(new Node(new Op.CallOp("defined", 1), records.node));
  }

  /**
   * {@return {@code defined(record)}}
   *
   * @param record the record
   */
  public static IntExpr defined(RecordExpr record) {
    return Exprs.intExpr(new Node(new Op.CallOp("defined", 1), record.node));
  }

  /**
   * {@return {@code domain}, whose comprehension variable {@link Over#named} can name}
   *
   * @param <E> what the domain gives the body
   * @param domain the domain
   */
  public static <E> Over<E> over(Domain<E> domain) {
    return new Over<>(domain, null);
  }
}
