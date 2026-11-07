package dev.gecode.iris.build;

import dev.gecode.iris.build.BuildException.Kind;
import dev.gecode.iris.build.State.Forward;
import dev.gecode.iris.build.State.NameState;
import dev.gecode.iris.build.State.Undefined;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/**
 * The binding check: one walk over the statements, which knows the depth and presence of each
 * definition, so that {@code optional()} counts wherever it is given.
 */
final class Walk {
  /**
   * A block on the walk.
   *
   * @param block identifies this block; open blocks are numbered in nesting order
   * @param depth the repeats around this block
   * @param required whether its statements always execute
   */
  private record Frame(int block, int depth, boolean required) {
    Frame withRequired(boolean value) {
      return new Frame(block, depth, value);
    }

    Frame withDepth(int value) {
      return new Frame(block, value, required);
    }
  }

  private static final class Bound {
    int count;
    boolean required;

    /** The block of a binding, which always precedes what follows it there. */
    @Nullable Integer block;
  }

  /** A repeat's {@code store}, which runs when the repeat ends. */
  private record PendingStore(int stmt, int repeatBlock, Stmt.Definition store, Frame frame) {}

  /** A {@code type} statement, in the block whose end hides it. */
  private record TypeDeclaration(int stmt, int block) {}

  private final State state;
  private int current;
  private final Map<String, Bound> bound = new HashMap<>();
  private final List<Frame> frames = new ArrayList<>(List.of(new Frame(0, 0, true)));
  private int nextBlock;
  private final List<PendingStore> stores = new ArrayList<>();
  private final Map<String, List<TypeDeclaration>> types = new HashMap<>();

  private Walk(State state) {
    this.state = state;
  }

  /** Check every forward handle, record and named type against how it was declared. */
  static void check(State state) {
    Walk walk = new Walk(state);
    for (int index = 0; index < state.stmts.size(); index++) {
      Stmt stmt = state.stmts.get(index);
      walk.current = index;
      Frame frame = walk.frames.getLast();
      for (Mention mention : Stmt.mentions(stmt)) {
        if (mention instanceof Mention.TypeUse use) {
          walk.typeUse(use.type());
        }
      }
      walk.recordDefinitions(index, stmt, frame);
      walk.followBlocks(stmt, frame);
    }
    walk.finish();
  }

  private BuildException fail(String message) {
    throw BuildException.check(Kind.BINDING, message, state.sites.get(current));
  }

  private boolean isOpen(int block) {
    for (Frame frame : frames.reversed()) {
      if (frame.block == block) {
        return true;
      }
    }
    return false;
  }

  /** {@code frame} with the number of a block of its own. */
  private Frame ownBlock(Frame frame) {
    nextBlock++;
    return new Frame(nextBlock, frame.depth, frame.required);
  }

  private int enter(Frame frame) {
    Frame own = ownBlock(frame);
    frames.add(own);
    return own.block;
  }

  private int recordBinding(String name, Frame frame) {
    boolean precedes = bindingPrecedes(name);
    Bound entry = bound.computeIfAbsent(name, key -> new Bound());
    entry.count++;
    entry.required |= frame.required;
    if (!precedes) {
      entry.block = frame.block;
    }
    return entry.count;
  }

  /** Whether a binding of {@code name} always executes before the current statement. */
  private boolean bindingPrecedes(String name) {
    Bound entry = bound.get(name);
    return entry != null && entry.block != null && isOpen(entry.block);
  }

  /** A statement defines {@code name} in {@code frame}, with {@code levels} in all. */
  private void bind(String name, Tag value, @Nullable Integer levels, Frame frame) {
    NameState named = state.names.get(name);
    if (named instanceof Undefined) {
      fail(
          "`"
              + name
              + "` comes from `undefined`, so nothing may define it; declare it with a handle"
              + " factory such as `int0(name)` or with `forward` instead");
    }
    if (!(named instanceof Forward forward)) {
      return;
    }
    Shape shape = forward.shape();
    if (shape.agrees(value, levels)) {
      recordBinding(name, frame);
    } else if (!bindingPrecedes(name)) {
      // After a binding, this is an explicit redefinition, which leaves the handle stale.
      fail(
          "the handle `"
              + name
              + "` is "
              + Shape.describe(shape.value(), shape.levels(), "any")
              + ", but this definition is "
              + Shape.describe(value, levels, "an unknown")
              + "; declare it with the definition's type and depth, or with `dynamic(name)`");
    }
  }

  /** A statement other than {@code store} defines {@code definition} in {@code frame}. */
  private void definition(Stmt.Definition definition, Frame frame) {
    String name = definition.name();
    if (name == null) {
      return;
    }
    if (name.equals("_")) {
      fail(
          "only `store` defines the entry named `_`: a declaration, a list or a capture discards"
              + " it, and `let` and `param` reject it");
    }
    Integer levels = definition.levels() == null ? null : frame.depth + definition.levels();
    bind(name, definition.value(), levels, frame);
  }

  /**
   * A {@code store} defines {@code store} in {@code frame}. No repeat collects {@code _} into a
   * column, so inside one it lasts for the iteration and leaves no entry.
   */
  private void store(Stmt.Definition store, Frame frame) {
    String name = store.name();
    if (name == null) {
      return;
    }
    if (name.equals("_") && frame.depth > 0) {
      bind(name, store.value(), store.levels(), frame.withRequired(false));
      return;
    }
    Integer levels = store.levels() == null ? null : frame.depth + store.levels();
    bind(name, store.value(), levels, frame);
  }

  private void pattern(List<Piece> pieces, Frame frame) {
    for (Piece piece : pieces) {
      if (piece instanceof Piece.Bind bindPiece) {
        definition(new Stmt.Definition(bindPiece.handle().name(), bindPiece.value(), 0), frame);
      }
    }
  }

  /** A statement uses the named type {@code named}, which must be the one visible. */
  private void typeUse(Declared.NamedSpec named) {
    List<TypeDeclaration> declarations =
        types.computeIfAbsent(named.name(), key -> new ArrayList<>());
    while (!declarations.isEmpty() && !isOpen(declarations.getLast().block)) {
      declarations.removeLast();
    }
    if (declarations.isEmpty() || declarations.getLast().stmt != named.declaration()) {
      throw BuildException.check(
          Kind.SCOPE,
          "the declaration of the type `"
              + named.name()
              + "` that this type came from is not the one visible here",
          state.sites.get(current));
    }
  }

  private void finish() {
    for (State.DeclaredName declaredName : state.declared) {
      String name = declaredName.name();
      Bound entry = bound.getOrDefault(name, new Bound());
      if (state.names.get(name) instanceof Forward forward) {
        if (entry.count == 0) {
          throw BuildException.check(
              Kind.BINDING,
              "the handle `"
                  + name
                  + "` is never defined; define it, or use `undefined` for a name left undefined",
              declaredName.site());
        }
        if (forward.shape().required() && !entry.required) {
          throw BuildException.check(
              Kind.BINDING,
              "the handle `"
                  + name
                  + "` is required, but every definition may not execute; declare it with"
                  + " `.optional()`, as in `int0(name).optional()`",
              declaredName.site());
        }
      }
    }
    for (State.RecordInfo record : state.records) {
      if (!record.bound) {
        throw BuildException.check(
            Kind.BINDING,
            "the record "
                + RecordName.describe(record.each)
                + " is never bound; pass it to the repeat whose records it names",
            record.firstUse);
      }
    }
  }

  /** Record what {@code stmt}, at {@code index}, defines in {@code frame}. */
  private void recordDefinitions(int index, Stmt stmt, Frame frame) {
    switch (stmt) {
      case Stmt.Param p -> definition(p.definition(), frame);
      case Stmt.Declaration d -> definition(d.definition(), frame);
      case Stmt.ListStmt l -> definition(l.definition(), frame);
      case Stmt.Let l -> definition(l.definition(), frame);
      case Stmt.ElseDeclaration e ->
          definition(e.definition(), ownBlock(frame.withRequired(false)));
      case Stmt.TypeDefinition t ->
          types
              .computeIfAbsent(t.name(), key -> new ArrayList<>())
              .add(new TypeDeclaration(index, frame.block));
      case Stmt.Line l -> pattern(l.pieces(), frame);
      case Stmt.Sections s -> {
        // End of file and `last` end sections without storing.
        Frame optional = ownBlock(frame.withRequired(false));
        for (Stmt.Clause clause : s.clauses()) {
          if (clause instanceof Stmt.Terminated terminated && terminated.store() != null) {
            store(terminated.store(), optional);
          }
        }
      }
      default -> {}
    }
  }

  /** Open or close the block {@code stmt} starts or ends, and check an arm's pattern. */
  private void followBlocks(Stmt stmt, Frame frame) {
    switch (stmt) {
      case Stmt.Close c -> {
        Frame closedFrame = frames.removeLast();
        if (!stores.isEmpty() && stores.getLast().repeatBlock == closedFrame.block) {
          PendingStore pending = stores.removeLast();
          current = pending.stmt;
          store(pending.store, pending.frame);
        }
      }
      case Stmt.Repeat r -> {
        int repeatBlock = enter(frame.withDepth(frame.depth + 1));
        if (r.count() instanceof Stmt.Terminated terminated && terminated.store() != null) {
          stores.add(new PendingStore(current, repeatBlock, terminated.store(), frame));
        }
      }
      case Stmt.If i -> enter(frame.withRequired(false));
      case Stmt.ElseIf e -> enter(frame.withRequired(false));
      case Stmt.Else e -> enter(frame.withRequired(false));
      case Stmt.Arm a -> {
        Frame arm = ownBlock(frame.withRequired(frame.required && !a.optional()));
        pattern(a.pieces(), arm);
        if (a.body()) {
          frames.add(arm);
        }
      }
      default -> {
        if (Stmt.opensBlock(stmt)) {
          enter(frame);
        }
      }
    }
  }
}
