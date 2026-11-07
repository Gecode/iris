package dev.gecode.iris.build;

import dev.gecode.iris.build.BuildException.Kind;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;
import org.jspecify.annotations.Nullable;

/** A builder's state: the statement stream and what its names and records are. */
final class State {
  private static final AtomicLong OWNERS = new AtomicLong();

  /** The builder id. It only compares handles; it is never printed. */
  final long id = OWNERS.incrementAndGet();

  final List<Stmt> stmts = new ArrayList<>();

  /** The call site of each statement. A {@code }} has the call that opened its block. */
  final List<StackTraceElement> sites = new ArrayList<>();

  /** The ids of the open blocks, innermost last. */
  private final List<Integer> open = new ArrayList<>();

  private int nextBlock;

  final Map<String, NameState> names = new LinkedHashMap<>();

  /** Declared, forward and undefined names in declaration order, with where they were declared. */
  final List<DeclaredName> declared = new ArrayList<>();

  final Set<String> types = new HashSet<>();

  final List<RecordInfo> records = new ArrayList<>();

  private final IdentityHashMap<RecordName, Integer> recordIds = new IdentityHashMap<>();

  /** The block that the last {@code }} closed, for {@code else} to continue. */
  @Nullable ClosedBlock closed;

  /** The first failed check, which {@code build()} throws again. */
  @Nullable BuildException issue;

  /** What a name is. */
  sealed interface NameState {}

  /**
   * A name that a handle declared, whose definitions must agree with {@code shape}.
   *
   * @param shape the promise
   * @param declaredBy the declared handle that declared it, or null for {@code forward}
   */
  record Forward(Shape shape, @Nullable Handle declaredBy) implements NameState {}

  /** From {@code undefined}; nothing may define it. */
  record Undefined() implements NameState {}

  record DeclaredName(String name, StackTraceElement site) {}

  /** The names a repeat's records print with, and whether a repeat binds them. */
  static final class RecordInfo {
    final @Nullable String each;
    final @Nullable String list;
    final StackTraceElement firstUse;
    boolean bound;

    RecordInfo(@Nullable String each, @Nullable String list, StackTraceElement firstUse) {
      this.each = each;
      this.list = list;
      this.firstUse = firstUse;
    }
  }

  record ClosedBlock(int parent, int opener, int close) {}

  /** The kinds of block, which decide the statements they take. */
  enum BlockKind {
    ROOT,
    INSTANCES,
    REPEAT,
    BRANCH,
    SECTIONS,
    ARM
  }

  /** An open block. */
  record Block(State state, int id, BlockKind kind, int depth, boolean required) {}

  Block root() {
    return openBlock(BlockKind.ROOT, 0, true);
  }

  BuildException fail(Kind kind, String message, StackTraceElement site) {
    BuildException error = BuildException.check(kind, message, site);
    if (issue == null) {
      issue = error;
    }
    throw error;
  }

  Block openBlock(BlockKind kind, int depth, boolean required) {
    int block = nextBlock++;
    open.add(block);
    return new Block(this, block, kind, depth, required);
  }

  void closeBlock(Block parent, int opener, StackTraceElement site) {
    open.removeLast();
    stmts.add(new Stmt.Close());
    sites.add(site);
    closed = new ClosedBlock(parent.id(), opener, stmts.size() - 1);
  }

  /** Check the mentions of {@code stmt} and append it to {@code block}. */
  int push(Block block, Stmt stmt, StackTraceElement site) {
    if (open.isEmpty() || open.getLast() != block.id()) {
      fail(Kind.SCOPE, "a statement went to a block that is not the innermost open one", site);
    }
    for (Mention mention : Stmt.mentions(stmt)) {
      check(mention, site);
    }
    stmts.add(stmt);
    sites.add(site);
    return stmts.size() - 1;
  }

  private void check(Mention mention, StackTraceElement site) {
    switch (mention) {
      case Mention.HandleUse use -> use(use.handle(), site);
      case Mention.TypeUse use -> {
        if (use.type().owner() != id) {
          fail(
              Kind.PROVENANCE,
              "the type `" + use.type().name() + "` does not come from this builder",
              site);
        }
      }
      case Mention.Expression expression -> {
        for (Op op : expression.ops()) {
          switch (op) {
            case Op.NameOp name -> use(name.handle(), site);
            case Op.MemberOp member -> use(member.handle(), site);
            case Op.RecordOp record -> recordOf(record.record(), site);
            case Op.RecordListOp record -> recordOf(record.record(), site);
            case Op.ListMemberOp record -> recordOf(record.record(), site);
            default -> {}
          }
        }
      }
    }
  }

  /**
   * Check a handle that a statement uses. The first use of a declared handle declares its name;
   * every other handle must come from this builder.
   */
  void use(Handle handle, StackTraceElement site) {
    String name = handle.name();
    if (handle.origin() != Handle.Origin.DECLARED) {
      if (handle.owner() != id) {
        fail(Kind.PROVENANCE, "the handle `" + name + "` does not come from this builder", site);
      }
      return;
    }
    NameState state = names.get(name);
    if (state == null) {
      names.put(name, new Forward(handle.shape(), handle));
      declared.add(new DeclaredName(name, site));
    } else if (!(state instanceof Forward forward && handle.equals(forward.declaredBy()))) {
      fail(
          Kind.REDEFINITION,
          "`"
              + name
              + "` is already declared by another handle in this builder; this one is "
              + handle.describe()
              + (handle.required() ? "" : " and optional")
              + ": use one handle for each name",
          site);
    }
  }

  /** Declare a name for {@code forward} or {@code undefined}. */
  void declare(String name, NameState state, StackTraceElement site) {
    if (names.containsKey(name)) {
      fail(Kind.REDEFINITION, "`" + name + "` is already declared", site);
    }
    names.put(name, state);
    declared.add(new DeclaredName(name, site));
  }

  /** The record of {@code record} in this builder, created at its first use. */
  int recordOf(RecordName record, StackTraceElement site) {
    Integer known = recordIds.get(record);
    if (known != null) {
      return known;
    }
    records.add(new RecordInfo(record.each, record.list, site));
    recordIds.put(record, records.size() - 1);
    return records.size() - 1;
  }

  /** A record for a repeat that nothing names. */
  int anonymousRecord(StackTraceElement site) {
    RecordInfo info = new RecordInfo(null, null, site);
    info.bound = true;
    records.add(info);
    return records.size() - 1;
  }

  /** Bind {@code record} to the repeat being pushed. */
  int bind(RecordName record, StackTraceElement site) {
    int index = recordOf(record, site);
    RecordInfo info = records.get(index);
    if (info.bound) {
      fail(
          Kind.BINDING,
          "the record "
              + record.describe()
              + " is already bound to a repeat; a record names the records of one repeat",
          site);
    }
    info.bound = true;
    return index;
  }

  /** The names that internal names must avoid. */
  Set<String> taken() {
    Set<String> taken = new HashSet<>(names.keySet());
    taken.addAll(types);
    for (RecordInfo record : records) {
      if (record.each != null) {
        taken.add(record.each);
      }
      if (record.list != null) {
        taken.add(record.list);
      }
    }
    return taken;
  }

  /** The handle the name of a definition through {@code handle} has afterwards. */
  DynField reference(String name) {
    return new DynField(Handle.reference(name, id));
  }

  /** The index of a record that a statement already used. */
  int recordIndex(RecordName record) {
    Integer index = recordIds.get(record);
    if (index == null) {
      throw new IllegalStateException("a record was printed before any statement used it");
    }
    return index;
  }
}
