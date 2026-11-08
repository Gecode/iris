// The statements of a builder and the checks made when they are written.

#include "state.hpp"

#include <atomic>
#include <memory>
#include <mutex>
#include <set>
#include <string>
#include <utility>

namespace iris::build::detail {

namespace {

constexpr BuildErrorKind scope_kind = BuildErrorKind::Scope;
constexpr BuildErrorKind provenance_kind = BuildErrorKind::Provenance;
constexpr BuildErrorKind redefinition_kind = BuildErrorKind::Redefinition;
constexpr BuildErrorKind unprintable_kind = BuildErrorKind::Unprintable;
constexpr BuildErrorKind binding_kind = BuildErrorKind::Binding;

void check_innermost(State &state, std::size_t block, Site site) {
  if (state.open.back() != block)
    state.report(scope_kind,
                 "a statement went to a scope that is not the innermost open "
                 "block; use the scope of the closure it is in",
                 site);
}

void check_owner(State &state, BuilderId owner, std::string_view what,
                 std::string_view name, Site site) {
  if (owner != state.id)
    state.report(provenance_kind,
                 std::string(what) + " `" + std::string(name) +
                     "` does not come from this builder",
                 site);
}

void check_record(State &state, BuilderId owner, Site site) {
  if (owner != state.id)
    state.report(provenance_kind,
                 "a record handle does not come from this builder", site);
}

void check_ops(State &state, const Ops &ops, Site site) {
  for (const Op &op : ops) {
    switch (op.kind) {
    case OpKind::Name:
    case OpKind::Member:
      check_owner(state, op.owner, "the handle", op.name, site);
      break;
    case OpKind::Record:
    case OpKind::RecordList:
    case OpKind::ListMember:
      check_record(state, op.owner, site);
      break;
    default:
      break;
    }
  }
}

void check_mention(State &state, const Mention &mention, Site site) {
  switch (mention.kind) {
  case Mention::Kind::Expression:
    check_ops(state, *mention.ops, site);
    break;
  case Mention::Kind::Name:
    check_owner(state, mention.owner, "the handle", mention.name, site);
    break;
  case Mention::Kind::Type:
    check_owner(state, mention.owner, "the type", mention.name, site);
    break;
  }
}

/// Check the handles of `stmt` and append it to the block `block`
std::size_t push(State &state, std::size_t block, Stmt stmt, Site site) {
  check_innermost(state, block, site);
  mentions(stmt, [&](const Mention &mention) {
    check_mention(state, mention, site);
  });
  state.stmts.push_back(std::move(stmt));
  state.sites.push_back(site);
  return state.stmts.size() - 1;
}

Stmt make(StmtKind kind) {
  Stmt stmt;
  stmt.kind = kind;
  return stmt;
}

/// Check a name that may be `_`, and intern it
std::string_view spelled_name(State &state, std::string_view name, Site site) {
  if (name.starts_with("gecode."))
    state.report(unprintable_kind,
                 "a definition cannot be named `" + std::string(name) +
                     "`: the prefix `gecode.` is reserved",
                 site);
  else if (!is_identifier(name))
    state.report(unprintable_kind,
                 "`" + std::string(name) + "` is not an Iris name", site);
  return intern(name);
}

/// Check the name of a definition, and intern it. A declaration, list or
/// capture named `_` stores nothing, and `let`, `param` and `type` reject
/// `_`, so no definition gives a handle for it.
std::string_view definition_name(State &state, std::string_view name,
                                 Site site) {
  if (name == "_")
    state.report(unprintable_kind,
                 "`_` names nothing here: use `discard` to read without "
                 "storing, or `forward(\"_\")` with `store` for the entry "
                 "named `_`",
                 site);
  return spelled_name(state, name, site);
}

/// Resolve a definition's target
Definition define(State &state, const Target &target, ValueTag value,
                  std::optional<std::size_t> levels, Site site) {
  Definition definition{.name = std::nullopt, .value = value, .levels = levels};
  switch (target.kind) {
  case Target::Kind::None:
  case Target::Kind::Discard:
    break;
  case Target::Kind::New: {
    const std::string_view name = definition_name(state, target.name, site);
    if (state.names.contains(name))
      state.report(redefinition_kind,
                   "`" + std::string(name) +
                       "` is already defined; to redefine it, pass its handle "
                       "as the target, or use `set`",
                   site);
    state.names[name] = NameState{};
    definition.name = name;
    break;
  }
  case Target::Kind::Existing:
    check_owner(state, target.owner, "the handle", target.name, site);
    definition.name = intern(target.name);
    break;
  }
  return definition;
}

/// Declare a checked name for a forward handle, a capture, or `undefined`
std::string_view declare(State &state, std::string_view name,
                         NameState name_state, Site site) {
  if (state.names.contains(name))
    state.report(redefinition_kind,
                 "`" + std::string(name) + "` is already declared", site);
  state.names[name] = std::move(name_state);
  state.declared.push_back(Declaration{name, site});
  return name;
}

/// The definition a terminator's `store` makes
Definition store_definition(State &state, const Target &target, Site site) {
  if (target.kind == Target::Kind::Discard)
    state.report(unprintable_kind,
                 "`store _` keeps an entry named `_`, so `discard` cannot "
                 "spell it: pass a handle from `forward(\"_\")`",
                 site);
  return define(state, target, ValueTag::Str, 0, site);
}

std::size_t new_record(State &state) {
  state.records.emplace_back();
  return state.records.size() - 1;
}

HandleName handle(const State &state, const Definition &definition) {
  return HandleName{definition.name.value_or(std::string_view()), state.id};
}

} // namespace

bool Stmt::opens_block() const {
  switch (kind) {
  case StmtKind::Repeat:
  case StmtKind::Sections:
  case StmtKind::If:
  case StmtKind::ElseIf:
  case StmtKind::Else:
  case StmtKind::Instances:
    return true;
  case StmtKind::Arm:
    return body;
  default:
    return false;
  }
}

void State::report(BuildErrorKind kind, std::string message, Site site) {
  if (!issue)
    issue = Issue{kind, std::move(message), site};
}

bool is_identifier(std::string_view name) {
  if (name.empty())
    return false;
  const auto alpha = [](char c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_';
  };
  if (!alpha(name.front()))
    return false;
  for (const char c : name.substr(1))
    if (const bool digit = c >= '0' && c <= '9'; !digit && !alpha(c))
      return false;
  return true;
}

void append(Ops &to, Ops &&from) {
  if (to.empty()) {
    to = std::move(from);
    return;
  }
  to.insert(to.end(), std::make_move_iterator(from.begin()),
            std::make_move_iterator(from.end()));
}

void append_text(std::vector<Piece> &pieces, std::string_view text) {
  if (text.empty())
    return;
  if (!pieces.empty() && pieces.back().kind == Piece::Kind::Text) {
    pieces.back().text += text;
    return;
  }
  Piece piece;
  piece.kind = Piece::Kind::Text;
  piece.text = std::string(text);
  pieces.push_back(std::move(piece));
}

std::string_view intern(std::string_view name) {
  // One copy of each distinct name, kept for the life of the process:
  // handles are literal types that outlive their builder.
  static std::mutex mutex;
  static std::set<std::string, std::less<>> names;
  const std::scoped_lock lock(mutex);
  auto found = names.find(name);
  if (found == names.end())
    found = names.emplace(name).first;
  return *found;
}

std::uint64_t next_binding() {
  static std::atomic<std::uint64_t> next{1};
  return next.fetch_add(1, std::memory_order_relaxed);
}

std::shared_ptr<State> new_state() {
  static std::atomic<BuilderId> next{1};
  auto state = std::make_shared<State>();
  state->id = next.fetch_add(1, std::memory_order_relaxed);
  return state;
}

BuilderId builder_id(const State &state) { return state.id; }

std::size_t statement_count(const State &state) { return state.stmts.size(); }

std::size_t open_block(State &state) {
  const std::size_t block = state.next_block++;
  state.open.push_back(block);
  return block;
}

void close_block(State &state, Site site) {
  state.open.pop_back();
  state.stmts.push_back(make(StmtKind::Close));
  state.sites.push_back(site);
}

void check_last(State &state, std::size_t statement, Site site) {
  if (statement + 1 != state.stmts.size())
    state.report(scope_kind,
                 "a branch was continued after later statements; continue an "
                 "`if` directly after its block",
                 site);
}

void simple(State &state, std::size_t block, Simple kind,
            std::vector<std::string> values, Site site) {
  StmtKind stmt_kind = StmtKind::Format;
  switch (kind) {
  case Simple::Format:
    stmt_kind = StmtKind::Format;
    break;
  case Simple::Comment:
    stmt_kind = StmtKind::Comment;
    break;
  case Simple::Stop:
    stmt_kind = StmtKind::Stop;
    break;
  case Simple::Tokens:
    stmt_kind = StmtKind::Tokens;
    break;
  case Simple::Separators:
    stmt_kind = StmtKind::Separators;
    break;
  case Simple::IgnoreTrailing:
    stmt_kind = StmtKind::IgnoreTrailing;
    break;
  case Simple::Expect:
    stmt_kind = StmtKind::Expect;
    break;
  case Simple::SkipLine:
    stmt_kind = StmtKind::SkipLine;
    break;
  case Simple::Instances:
    stmt_kind = StmtKind::Instances;
    break;
  case Simple::Else:
    stmt_kind = StmtKind::Else;
    break;
  }
  Stmt stmt = make(stmt_kind);
  stmt.strings = std::move(values);
  push(state, block, std::move(stmt), site);
}

std::size_t computed(State &state, std::size_t block, Computed kind, Ops &&ops,
                     Site site) {
  StmtKind stmt_kind = StmtKind::SkipLines;
  switch (kind) {
  case Computed::SkipLines:
    stmt_kind = StmtKind::SkipLines;
    break;
  case Computed::Index:
    stmt_kind = StmtKind::Index;
    break;
  case Computed::If:
    stmt_kind = StmtKind::If;
    break;
  case Computed::ElseIf:
    stmt_kind = StmtKind::ElseIf;
    break;
  }
  Stmt stmt = make(stmt_kind);
  stmt.ops = std::move(ops);
  return push(state, block, std::move(stmt), site);
}

void check(State &state, std::size_t block, Ops &&condition,
           std::optional<std::string> message, Site site) {
  Stmt stmt = make(StmtKind::Check);
  stmt.ops = std::move(condition);
  stmt.message = std::move(message);
  push(state, block, std::move(stmt), site);
}

void pattern_statement(State &state, std::size_t block, bool find,
                       std::vector<Piece> &&pieces, Site site) {
  Stmt stmt = make(find ? StmtKind::Find : StmtKind::Line);
  stmt.pattern = std::move(pieces);
  push(state, block, std::move(stmt), site);
}

HandleName declaration(State &state, std::size_t block, const Target &target,
                       ValueTag value, Declared &&declared, Site site) {
  Stmt stmt = make(StmtKind::Declaration);
  stmt.definition =
      define(state, target, value, declared.dimensions.size(), site);
  stmt.declared = std::move(declared);
  const HandleName name = handle(state, stmt.definition);
  push(state, block, std::move(stmt), site);
  return name;
}

HandleName else_declaration(State &state, std::size_t block, std::size_t after,
                            const Target &target, ValueTag value,
                            Declared &&declared, Site site) {
  check_last(state, after, site);
  Stmt stmt = make(StmtKind::ElseDeclaration);
  stmt.definition =
      define(state, target, value, declared.dimensions.size(), site);
  if (stmt.definition.name != std::optional<std::string_view>("if"))
    state.report(unprintable_kind,
                 "`else_read_if` declares the name `if`, which its target "
                 "must have",
                 site);
  stmt.declared = std::move(declared);
  const HandleName name = handle(state, stmt.definition);
  push(state, block, std::move(stmt), site);
  return name;
}

HandleName list(State &state, std::size_t block, const Target &target,
                ValueTag value, Declared &&declared, ListEnd &&end, Site site) {
  Stmt stmt = make(StmtKind::List);
  stmt.definition = define(state, target, value, 1, site);
  stmt.declared = std::move(declared);
  stmt.end = std::move(end);
  const HandleName name = handle(state, stmt.definition);
  push(state, block, std::move(stmt), site);
  return name;
}

HandleName let(State &state, std::size_t block, const Target &target,
               ValueTag value, std::optional<std::size_t> levels, Ops &&ops,
               Site site) {
  Stmt stmt = make(StmtKind::Let);
  stmt.definition = define(state, target, value, levels, site);
  stmt.ops = std::move(ops);
  const HandleName name = handle(state, stmt.definition);
  push(state, block, std::move(stmt), site);
  return name;
}

HandleName param(State &state, std::size_t block, const Target &target,
                 ValueTag value, std::optional<std::size_t> levels,
                 std::optional<Ops> &&fallback, Site site) {
  Stmt stmt = make(StmtKind::Param);
  stmt.definition = define(state, target, value, levels, site);
  stmt.ops = std::move(fallback);
  const HandleName name = handle(state, stmt.definition);
  push(state, block, std::move(stmt), site);
  return name;
}

TypeName define_type(State &state, std::size_t block, std::string_view name,
                     Declared &&declared, Site site) {
  const std::string_view interned = definition_name(state, name, site);
  state.types.insert(interned);
  Stmt stmt = make(StmtKind::TypeDefinition);
  stmt.type_name = interned;
  stmt.declared = std::move(declared);
  const std::size_t index = push(state, block, std::move(stmt), site);
  return TypeName{interned, state.id, index};
}

std::size_t repeat(State &state, std::size_t block, RepeatKind kind,
                   Ops &&times, std::vector<std::string> terminators,
                   const Target &store, Site site) {
  const std::size_t record = new_record(state);
  Stmt stmt = make(StmtKind::Repeat);
  stmt.repeat = kind;
  stmt.record = record;
  if (kind == RepeatKind::Times)
    stmt.ops = std::move(times);
  stmt.strings = std::move(terminators);
  if (store.kind != Target::Kind::None)
    stmt.store = store_definition(state, store, site);
  push(state, block, std::move(stmt), site);
  return record;
}

void sections(State &state, std::size_t block, std::vector<Clause> clauses,
              Site site) {
  Stmt stmt = make(StmtKind::Sections);
  for (Clause &clause : clauses) {
    std::optional<Definition> store;
    if (clause.store.kind != Target::Kind::None)
      store = store_definition(state, clause.store, site);
    if (!clause.until.empty())
      stmt.clauses.push_back(
          SectionClause{.skip_other = false,
                        .terminators = std::move(clause.until),
                        .store = store});
    else if (store)
      state.report(unprintable_kind, "`store` needs an `until` before it",
                   site);
    if (clause.skip_other)
      stmt.clauses.push_back(SectionClause{
          .skip_other = true, .terminators = {}, .store = std::nullopt});
  }
  push(state, block, std::move(stmt), site);
}

void arm(State &state, std::size_t block, std::vector<Piece> &&pieces,
         bool optional, bool last, bool body, Site site) {
  Stmt stmt = make(StmtKind::Arm);
  stmt.pattern = std::move(pieces);
  stmt.optional = optional;
  stmt.last = last;
  stmt.body = body;
  push(state, block, std::move(stmt), site);
}

HandleName capture(State &state, std::string_view name, ValueTag value,
                   std::optional<Declared> &&type,
                   std::optional<std::size_t> levels, bool required,
                   Site site) {
  // The handles of the capture's type print later, in a pattern.
  if (type)
    mention_declared(*type, [&](const Mention &mention) {
      check_mention(state, mention, site);
    });
  const std::string_view interned = definition_name(state, name, site);
  NameState name_state;
  name_state.kind = NameState::Kind::Capture;
  name_state.shape = Shape{value, levels, required};
  name_state.spec = std::move(type);
  return HandleName{declare(state, interned, std::move(name_state), site),
                    state.id};
}

HandleName forward(State &state, std::string_view name, ValueTag value,
                   std::optional<std::size_t> levels, bool required,
                   Site site) {
  const std::string_view interned = spelled_name(state, name, site);
  NameState name_state;
  name_state.kind = NameState::Kind::Forward;
  name_state.shape = Shape{value, levels, required};
  return HandleName{declare(state, interned, std::move(name_state), site),
                    state.id};
}

HandleName undefined(State &state, std::string_view name, Site site) {
  const std::string_view interned = spelled_name(state, name, site);
  NameState name_state;
  name_state.kind = NameState::Kind::Undefined;
  return HandleName{declare(state, interned, std::move(name_state), site),
                    state.id};
}

std::size_t forward_record(State &state, std::string_view each,
                           std::string_view list, Site site) {
  const std::string_view each_name = spelled_name(state, each, site);
  const std::string_view list_name = spelled_name(state, list, site);
  const std::size_t id = new_record(state);
  RecordInfo &record = state.records[id];
  record.each = each_name;
  record.list = list_name;
  record.forward = site;
  return id;
}

void name_record(State &state, std::size_t record, bool each,
                 std::string_view name, Site site) {
  const std::string_view interned = spelled_name(state, name, site);
  RecordInfo &info = state.records[state.record_id(record)];
  (each ? info.each : info.list) = interned;
}

void record_as(State &state, std::size_t record, std::size_t forward,
               BuilderId owner, Site site) {
  check_record(state, owner, site);
  if (owner != state.id || forward >= state.records.size() ||
      !state.records[forward].forward || state.records[forward].bound) {
    state.report(binding_kind,
                 "`record_as` needs a record from `forward_record` that is "
                 "not bound yet",
                 site);
    return;
  }
  if (state.records[record].link) {
    state.report(binding_kind,
                 "this repeat is already bound to a forward record; a repeat "
                 "takes one `record_as`",
                 site);
    return;
  }
  state.records[forward].bound = true;
  state.records[record].link = forward;
}

} // namespace iris::build::detail
