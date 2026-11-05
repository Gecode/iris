package dev.gecode.iris.internal.read;

import dev.gecode.iris.ErrorCode;
import dev.gecode.iris.internal.read.Domain.Bounds;
import dev.gecode.iris.internal.syntax.Ast.Expr;
import dev.gecode.iris.internal.syntax.Ast.ReadingType;
import dev.gecode.iris.internal.syntax.Ast.Stmt;
import java.util.List;
import org.jspecify.annotations.Nullable;

/** The ranges of reading types, which expressions give and type statements fix (§6.4, §6.5). */
final class Domains {
  private final Execution execution;
  private final Evaluator evaluator;
  private final List<Stmt.TypeDefinition> types;

  /** The bounds of each type statement run so far, by its id. */
  private final @Nullable Bounds[] definitions;

  Domains(Execution execution, Evaluator evaluator, List<Stmt.TypeDefinition> types) {
    this.execution = execution;
    this.evaluator = evaluator;
    this.types = types;
    this.definitions = new Bounds[types.size()];
  }

  /** Runs the type statement {@code definition}, which fixes the range of the type. */
  void define(Stmt.TypeDefinition definition) {
    definitions[definition.id()] = bounds(definition.type());
  }

  /** Returns {@code type} with its range evaluated. */
  Domain of(ReadingType type) {
    String name = type instanceof ReadingType.Named named ? definition(named).name() : null;
    return new Domain(type.kind(), name, bounds(type));
  }

  private Stmt.TypeDefinition definition(ReadingType.Named named) {
    return types.get(named.definition());
  }

  private Bounds bounds(ReadingType type) {
    return switch (type) {
      case ReadingType.Named named -> {
        Bounds bounds = definitions[named.definition()];
        if (bounds == null) {
          throw execution.failure(
              ErrorCode.TYPE_UNDEFINED,
              "the type '" + definition(named).name() + "' is not defined yet");
        }
        yield bounds;
      }
      case ReadingType.Range range -> rangeBounds(range);
      case ReadingType.Base _, ReadingType.Alphabet _ -> Bounds.NONE;
    };
  }

  private Bounds rangeBounds(ReadingType.Range range) {
    Expr lowerBound = range.lower();
    Expr upperBound = range.upper();
    long lower = lowerBound == null ? Long.MIN_VALUE : bound(lowerBound);
    long upper = upperBound == null ? Long.MAX_VALUE : bound(upperBound);
    if (upperBound != null && range.exclusive()) {
      if (upper == Long.MIN_VALUE) {
        throw execution.failure(
            ErrorCode.INTEGER_OVERFLOW, "integer overflow in the upper bound of a range");
      }
      upper--;
    }
    return new Bounds(lowerBound != null, lower, upperBound != null, upper);
  }

  private long bound(Expr expression) {
    if (evaluator.evaluate(expression) instanceof Stored.Int bound) {
      return bound.value();
    }
    throw execution.failure(ErrorCode.RANGE_BOUNDS, "the bounds of a range must be integers");
  }
}
