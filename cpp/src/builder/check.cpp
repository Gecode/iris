// The binding check: one walk over the statements, which knows the depth and
// presence of each definition. The walk is a loop over the flat stream with
// a stack of open blocks, so its depth costs no host stack.

#include "state.hpp"

#include <algorithm>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

namespace iris::build::detail {

namespace {

struct Frame {
  /// Identifies this block. Open blocks are numbered in nesting order.
  std::size_t block = 0;
  /// The repeats around this block
  std::size_t depth = 0;
  bool required = true;
};

struct Bound {
  std::size_t count = 0;
  bool required = false;
  /// The block of a binding, which always precedes what follows it there
  std::optional<std::size_t> block;
};

/// A repeat's `store`, which runs when the repeat ends
struct PendingStore {
  /// The repeat statement, which the store belongs to
  std::size_t stmt = 0;
  std::size_t repeat_block = 0;
  Definition store;
  Frame frame;
};

/// A `type` statement, in the block whose end hides it
struct TypeDeclaration {
  std::size_t stmt = 0;
  std::size_t block = 0;
};

std::string_view describe(ValueTag value) {
  switch (value) {
  case ValueTag::Int:
    return "int";
  case ValueTag::Float:
    return "float";
  case ValueTag::Str:
    return "string";
  case ValueTag::Num:
    return "num";
  case ValueTag::Dyn:
    return "dynamic";
  }
  return "dynamic";
}

std::string describe(ValueTag value, std::optional<std::size_t> levels,
                     std::string_view open) {
  if (levels)
    return std::string(describe(value)) + " at depth " +
           std::to_string(*levels);
  return std::string(describe(value)) + " at " + std::string(open) + " depth";
}

/// Whether a definition certainly has the shape a forward handle promises.
/// A value type or depth that is not known agrees only with an open promise.
bool agrees(const Shape &promised, ValueTag value,
            std::optional<std::size_t> levels) {
  bool value_agrees = false;
  switch (promised.value) {
  case ValueTag::Dyn:
    value_agrees = true;
    break;
  case ValueTag::Num:
    value_agrees = value == ValueTag::Int || value == ValueTag::Float ||
                   value == ValueTag::Num;
    break;
  default:
    value_agrees = promised.value == value;
    break;
  }
  return value_agrees && (!promised.levels || promised.levels == levels);
}

class Walk {
public:
  explicit Walk(const State &state) : state_(state) {}

  std::optional<Issue> run() {
    for (std::size_t index = 0; index < state_.stmts.size(); index++) {
      current_ = index;
      const Stmt &stmt = state_.stmts[index];
      const Frame frame = frames_.back();
      mentions(stmt, [&](const Mention &mention) {
        if (mention.kind == Mention::Kind::Type && !issue_)
          type_use(mention);
      });
      if (!issue_)
        record_definitions(index, stmt, frame);
      if (!issue_)
        follow_blocks(stmt, frame);
      if (issue_)
        return issue_;
    }
    finish();
    return issue_;
  }

private:
  [[nodiscard]] Site site() const { return state_.sites[current_]; }

  /// Fail the statement being checked
  void fail(std::string message) { fail_at(site(), std::move(message)); }

  void fail_at(Site at, std::string message,
               BuildErrorKind kind = BuildErrorKind::Binding) {
    if (!issue_)
      issue_ = Issue{kind, std::move(message), at};
  }

  [[nodiscard]] bool is_open(std::size_t block) const {
    return std::ranges::binary_search(frames_, block, {}, &Frame::block);
  }

  /// `frame` with the number of a block of its own
  Frame own_block(Frame frame) {
    frame.block = ++next_block_;
    return frame;
  }

  /// Open a block and return its number
  std::size_t enter(Frame frame) {
    frame = own_block(frame);
    frames_.push_back(frame);
    return frame.block;
  }

  /// Whether a binding of `name` always executes before the current
  /// statement
  [[nodiscard]] bool binding_precedes(std::string_view name) const {
    const auto found = bound_.find(name);
    return found != bound_.end() && found->second.block &&
           is_open(*found->second.block);
  }

  std::size_t record_binding(std::string_view name, Frame frame) {
    const bool precedes = binding_precedes(name);
    Bound &bound = bound_[name];
    bound.count++;
    bound.required = bound.required || frame.required;
    if (!precedes)
      bound.block = frame.block;
    return bound.count;
  }

  /// A statement defines `name` in `frame`, with `levels` in all
  void bind(std::string_view name, ValueTag value,
            std::optional<std::size_t> levels, Frame frame) {
    const auto found = state_.names.find(name);
    if (found == state_.names.end())
      return;
    const NameState &name_state = found->second;
    if (name_state.kind == NameState::Kind::Undefined) {
      fail("`" + std::string(name) +
           "` comes from `undefined`, so nothing may define it; use "
           "`forward` for a name that is defined");
    } else if (name_state.kind == NameState::Kind::Forward) {
      const Shape &shape = name_state.shape;
      if (agrees(shape, value, levels))
        record_binding(name, frame);
      else if (!binding_precedes(name))
        // After a binding, another type is an explicit redefinition, which
        // leaves the handle stale.
        fail("the forward handle `" + std::string(name) + "` is " +
             describe(shape.value, shape.levels, "any") +
             ", but this definition is " +
             describe(value, levels, "an unknown") +
             "; declare it with the definition's type, or as a `DynField`");
    }
  }

  /// A statement other than `store` defines `definition` in `frame`
  void definition(const Definition &definition, Frame frame) {
    if (!definition.name)
      return;
    if (*definition.name == "_") {
      fail("only `store` defines the entry named `_`: a declaration, a list "
           "or a capture discards it, and `let` and `param` reject it");
      return;
    }
    std::optional<std::size_t> levels;
    if (definition.levels)
      levels = frame.depth + *definition.levels;
    bind(*definition.name, definition.value, levels, frame);
  }

  /// A `store` defines `store` in `frame`. No repeat collects `_` into a
  /// column, so inside one it lasts for the iteration and leaves no entry.
  void store(const Definition &store, Frame frame) {
    if (!store.name)
      return;
    if (*store.name == "_" && frame.depth > 0) {
      Frame local = frame;
      local.required = false;
      bind(*store.name, store.value, store.levels, local);
      return;
    }
    std::optional<std::size_t> levels;
    if (store.levels)
      levels = frame.depth + *store.levels;
    bind(*store.name, store.value, levels, frame);
  }

  void pattern(const std::vector<Piece> &pieces, Frame frame) {
    for (const Piece &piece : pieces) {
      if (issue_)
        return;
      if (piece.kind == Piece::Kind::Capture)
        capture(piece.name, frame);
      else if (piece.kind == Piece::Kind::Bind)
        definition(
            Definition{.name = piece.name, .value = piece.value, .levels = 0},
            frame);
    }
  }

  /// A pattern binds the capture `name` in `frame`
  void capture(std::string_view name, Frame frame) {
    const auto found = state_.names.find(name);
    if (found == state_.names.end() ||
        found->second.kind != NameState::Kind::Capture) {
      fail("`" + std::string(name) +
           "` is not a capture; use `bind` to define it in a pattern, or "
           "`ref` to match its value");
      return;
    }
    const NameState &name_state = found->second;
    if (record_binding(name, frame) > 1) {
      fail("the capture `" + std::string(name) +
           "` is bound twice; a capture belongs to one pattern, so use `ref` "
           "to match its value again");
      return;
    }
    const Shape &shape = name_state.shape;
    if (shape.levels && *shape.levels != frame.depth) {
      fail("the capture `" + std::string(name) +
           "` is bound in a different scope: create it with `capture` in the "
           "scope of the pattern");
      return;
    }
    if (shape.required != frame.required) {
      const std::string_view form =
          frame.required ? "capture" : "optional_capture";
      fail("the capture `" + std::string(name) +
           "` is bound where its presence differs: use `" + std::string(form) +
           "`");
      return;
    }
    if (name_state.spec)
      mention_declared(*name_state.spec, [&](const Mention &mention) {
        if (mention.kind == Mention::Kind::Type && !issue_)
          type_use(mention);
      });
  }

  /// A statement uses the named type `named`, which must be the one visible
  void type_use(const Mention &named) {
    std::vector<TypeDeclaration> &declarations = types_[named.name];
    while (!declarations.empty() && !is_open(declarations.back().block))
      declarations.pop_back();
    if (declarations.empty() || declarations.back().stmt != named.declaration)
      fail_at(site(),
              "the declaration of the type `" + std::string(named.name) +
                  "` that this handle came from is not the one visible here",
              BuildErrorKind::Scope);
  }

  void finish() {
    for (const Declaration &declaration : state_.declared) {
      const auto found = bound_.find(declaration.name);
      const Bound bound = found == bound_.end() ? Bound{} : found->second;
      const NameState &name_state = state_.names.at(declaration.name);
      const std::string name(declaration.name);
      if (name_state.kind == NameState::Kind::Capture && bound.count == 0) {
        fail_at(declaration.site, "the capture `" + name +
                                      "` is never bound; add it to a pattern");
        return;
      }
      if (name_state.kind == NameState::Kind::Forward) {
        if (bound.count == 0) {
          fail_at(declaration.site,
                  "the forward handle `" + name +
                      "` is never bound; define it, or use `undefined` for a "
                      "name left undefined");
          return;
        }
        if (name_state.shape.required && !bound.required) {
          fail_at(declaration.site,
                  "the forward handle `" + name +
                      "` is required, but every definition may not execute; "
                      "declare it with `Presence::Maybe`");
          return;
        }
      }
    }
    for (const RecordInfo &record : state_.records) {
      if (record.forward && !record.bound) {
        fail_at(*record.forward,
                "the forward record `" +
                    std::string(record.each.value_or(std::string_view())) +
                    "` is never bound; use `record_as` in its repeat");
        return;
      }
    }
  }

  /// Record what `stmt`, at `index`, defines in `frame`
  void record_definitions(std::size_t index, const Stmt &stmt, Frame frame) {
    switch (stmt.kind) {
    case StmtKind::Param:
    case StmtKind::Declaration:
    case StmtKind::List:
    case StmtKind::Let:
      definition(stmt.definition, frame);
      break;
    case StmtKind::ElseDeclaration: {
      Frame branch = frame;
      branch.required = false;
      definition(stmt.definition, own_block(branch));
      break;
    }
    case StmtKind::TypeDefinition:
      types_[stmt.type_name].push_back(TypeDeclaration{index, frame.block});
      break;
    case StmtKind::Line:
    case StmtKind::Find:
      pattern(stmt.pattern, frame);
      break;
    case StmtKind::Sections: {
      // End of file and `last` end sections without storing.
      Frame optional = frame;
      optional.required = false;
      optional = own_block(optional);
      for (const SectionClause &clause : stmt.clauses)
        if (clause.store && !issue_)
          store(*clause.store, optional);
      break;
    }
    default:
      break;
    }
  }

  /// Open or close the block `stmt` starts or ends, and check an arm's
  /// pattern in its own block
  void follow_blocks(const Stmt &stmt, Frame frame) {
    switch (stmt.kind) {
    case StmtKind::Close: {
      const Frame closed = frames_.back();
      frames_.pop_back();
      if (!stores_.empty() && stores_.back().repeat_block == closed.block) {
        const PendingStore pending = stores_.back();
        stores_.pop_back();
        current_ = pending.stmt;
        store(pending.store, pending.frame);
      }
      break;
    }
    case StmtKind::Repeat: {
      Frame inner = frame;
      inner.depth++;
      const std::size_t repeat_block = enter(inner);
      if (stmt.repeat == RepeatKind::Until && stmt.store)
        stores_.push_back(
            PendingStore{current_, repeat_block, *stmt.store, frame});
      break;
    }
    case StmtKind::If:
    case StmtKind::ElseIf:
    case StmtKind::Else: {
      Frame branch = frame;
      branch.required = false;
      enter(branch);
      break;
    }
    case StmtKind::Arm: {
      Frame arm = frame;
      arm.required = frame.required && !stmt.optional;
      arm = own_block(arm);
      pattern(stmt.pattern, arm);
      if (stmt.body)
        frames_.push_back(arm);
      break;
    }
    default:
      if (stmt.opens_block())
        enter(frame);
      break;
    }
  }

  const State &state_;
  /// The index of the statement being checked
  std::size_t current_ = 0;
  std::unordered_map<std::string_view, Bound> bound_;
  std::vector<Frame> frames_{Frame{}};
  std::size_t next_block_ = 0;
  /// The stores of the open repeats, innermost last
  std::vector<PendingStore> stores_;
  /// The declarations of each type name so far, latest last. Those of
  /// closed blocks are dropped when a use finds them.
  std::unordered_map<std::string_view, std::vector<TypeDeclaration>> types_;
  std::optional<Issue> issue_;
};

} // namespace

std::optional<Issue> check_bindings(const State &state) {
  return Walk(state).run();
}

} // namespace iris::build::detail
