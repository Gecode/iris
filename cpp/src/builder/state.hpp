#pragma once

// The statement stream of a builder: one flat list in text order, with blocks
// marked by openers and Close, so that every pass is a loop over it.

#include <iris/build.hpp>

#include <cstddef>
#include <cstdint>
#include <expected>
#include <optional>
#include <source_location>
#include <string>
#include <string_view>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace iris::build::detail {

/// What a definition defines: no name for the discard `_`. `levels` counts
/// the definition's own array levels, or is absent when they are open.
struct Definition {
  std::optional<std::string_view> name;
  ValueTag value = ValueTag::Str;
  std::optional<std::size_t> levels;
};

struct SectionClause {
  bool skip_other = false;
  std::vector<std::string> terminators;
  std::optional<Definition> store;
};

enum class StmtKind : std::uint8_t {
  Format,
  Comment,
  Stop,
  Tokens,
  Separators,
  IgnoreTrailing,
  Expect,
  Param,
  Declaration,
  List,
  TypeDefinition,
  Line,
  Find,
  SkipLine,
  SkipLines,
  Index,
  Let,
  Check,
  Repeat,
  Sections,
  Arm,
  If,
  ElseIf,
  Else,
  /// `else if:type`, a declaration named `if` as the else branch
  ElseDeclaration,
  Instances,
  Close,
};

/// A statement. Only the fields of its kind are set.
struct Stmt {
  StmtKind kind = StmtKind::Close;
  /// The words of Format (one), Comment, Stop, Tokens (one), Separators
  /// (one), Expect, and the terminators of a Repeat until
  std::vector<std::string> strings;
  /// The message of Check
  std::optional<std::string> message;
  /// What Param, Declaration, List, Let and ElseDeclaration define
  Definition definition;
  /// The type of Declaration, List, TypeDefinition and ElseDeclaration
  Declared declared;
  /// The end of List
  ListEnd end;
  /// The expression of SkipLines, Index, Let, Check, If, ElseIf, the count
  /// of a Repeat with RepeatKind::Times, and the default of Param
  std::optional<Ops> ops;
  /// The name of TypeDefinition
  std::string_view type_name;
  /// The pattern of Line, Find and Arm
  std::vector<Piece> pattern;
  /// The options of Arm
  bool optional = false;
  bool last = false;
  bool body = false;
  /// The count, record and store of Repeat
  RepeatKind repeat = RepeatKind::Times;
  std::size_t record = 0;
  std::optional<Definition> store;
  /// The clauses of Sections
  std::vector<SectionClause> clauses;

  /// Whether this statement starts a block that Close ends
  [[nodiscard]] bool opens_block() const;
  /// Whether this statement continues the line of the preceding `}`
  [[nodiscard]] bool continues_if() const {
    return kind == StmtKind::ElseIf || kind == StmtKind::Else ||
           kind == StmtKind::ElseDeclaration;
  }
};

/// Something a statement mentions: an expression, a handle's name, or a
/// named type
struct Mention {
  enum class Kind : std::uint8_t { Expression, Name, Type };
  Kind kind = Kind::Expression;
  const Ops *ops = nullptr;
  std::string_view name;
  BuilderId owner = 0;
  std::size_t declaration = 0;
};

/// The mention of an expression
inline Mention ops_mention(const Ops &ops) {
  Mention mention;
  mention.ops = &ops;
  return mention;
}

/// Visit every expression, name and type that `declared` mentions
template <class F>
void mention_declared(const Declared &declared, const F &visit) {
  for (const Ops &dimension : declared.dimensions)
    visit(ops_mention(dimension));
  if (declared.type.kind == TypeSpec::Kind::Named)
    visit(Mention{.kind = Mention::Kind::Type,
                  .ops = nullptr,
                  .name = declared.type.named,
                  .owner = declared.type.owner,
                  .declaration = declared.type.declaration});
  else if (declared.type.kind == TypeSpec::Kind::Range) {
    if (declared.type.lower)
      visit(ops_mention(*declared.type.lower));
    if (declared.type.upper)
      visit(ops_mention(*declared.type.upper));
  }
  if (declared.layout)
    visit(ops_mention(*declared.layout));
  if (declared.offset)
    visit(ops_mention(*declared.offset));
}

/// Whether a piece names a handle: a capture, a bind or a reference
inline bool names_handle(const Piece &piece) {
  return piece.kind == Piece::Kind::Capture ||
         piece.kind == Piece::Kind::Bind ||
         piece.kind == Piece::Kind::Reference;
}

/// Whether a piece of this kind may have a capture type
inline bool takes_capture_type(const Piece &piece) {
  return piece.kind == Piece::Kind::Bind ||
         piece.kind == Piece::Kind::Validate ||
         piece.kind == Piece::Kind::DiscardAs;
}

/// Visit every expression, name and type that `stmt` mentions
template <class F> void mentions(const Stmt &stmt, const F &visit) {
  switch (stmt.kind) {
  case StmtKind::Declaration:
  case StmtKind::List:
  case StmtKind::TypeDefinition:
  case StmtKind::ElseDeclaration:
    mention_declared(stmt.declared, visit);
    if (stmt.kind == StmtKind::List && !stmt.end.eol)
      visit(ops_mention(stmt.end.sentinel));
    break;
  case StmtKind::Line:
  case StmtKind::Find:
  case StmtKind::Arm:
    for (const Piece &piece : stmt.pattern) {
      if (names_handle(piece))
        visit(Mention{.kind = Mention::Kind::Name,
                      .ops = nullptr,
                      .name = piece.name,
                      .owner = piece.owner,
                      .declaration = 0});
      if (takes_capture_type(piece) && piece.type)
        mention_declared(*piece.type, visit);
    }
    break;
  default:
    if (stmt.ops)
      visit(ops_mention(*stmt.ops));
    break;
  }
}

/// The value type, levels and presence that a forward handle or capture
/// promises
struct Shape {
  ValueTag value = ValueTag::Dyn;
  std::optional<std::size_t> levels;
  bool required = true;
};

/// What the builder knows about a name
struct NameState {
  enum class Kind : std::uint8_t { Defined, Forward, Undefined, Capture };
  Kind kind = Kind::Defined;
  Shape shape;
  /// The type of a capture; none for an untyped one
  std::optional<Declared> spec;
};

struct RecordInfo {
  std::optional<std::string_view> each;
  std::optional<std::string_view> list;
  /// Where forward_record made it, awaiting record_as
  std::optional<Site> forward;
  bool bound = false;
  /// The forward record this repeat was bound to
  std::optional<std::size_t> link;
};

/// A forward handle or capture, with the call that made it
struct Declaration {
  std::string_view name;
  Site site;
};

/// A builder check that failed, recorded until build()
struct Issue {
  BuildErrorKind kind = BuildErrorKind::Scope;
  std::string message;
  Site site;
};

inline constexpr std::size_t root_block = 0;

/// The builder's state, shared by its scopes
struct State {
  BuilderId id = 0;
  std::vector<Stmt> stmts;
  /// The call site of each statement in `stmts`. A `}` has the call that
  /// wrote its block.
  std::vector<Site> sites;
  /// The ids of the open blocks, innermost last
  std::vector<std::size_t> open{root_block};
  std::size_t next_block = root_block + 1;
  std::unordered_map<std::string_view, NameState> names;
  /// Forward handles and captures in creation order, for reporting
  std::vector<Declaration> declared;
  std::unordered_set<std::string_view> types;
  std::vector<RecordInfo> records;
  std::optional<Issue> issue;

  /// Keep the first issue; the checks after it may only be its consequences
  void report(BuildErrorKind kind, std::string message, Site site);
  /// Follow a repeat's record to the forward record it was bound to
  [[nodiscard]] std::size_t record_id(std::size_t record) const {
    if (record >= records.size())
      return record;
    const std::optional<std::size_t> &link = records[record].link;
    return link ? *link : record;
  }
};

/// Whether `name` is an Iris identifier
bool is_identifier(std::string_view name);

/// The emitted text, and the start offset and call site of each statement
struct Printed {
  struct Entry {
    std::size_t offset = 0;
    Site site;
    bool close = false;
  };
  std::string text;
  std::vector<Entry> sites;
};

/// Check every capture, forward handle and named type against how it was
/// declared
std::optional<Issue> check_bindings(const State &state);

/// Print the statements as text, or return why they have no spelling
std::expected<Printed, Issue> print(const State &state);

} // namespace iris::build::detail
