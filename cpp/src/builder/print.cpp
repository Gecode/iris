// The printer. It writes what `iris fmt` writes with no line limit, so the
// text a builder emits is already formatted. Expressions print from a stack
// of tasks, so their depth costs no host stack.

#include "state.hpp"

#include "../numbers.hpp"

#include <algorithm>
#include <array>
#include <charconv>
#include <cmath>
#include <limits>
#include <span>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

namespace iris::build::detail {

namespace {

/// The words that end a range with no upper bound, from the Specification's
/// `upper bound ahead` predicate
/// The prefix of the host constants, as in `gecode.int_max`
constexpr std::string_view host_prefix = "gecode.";

constexpr std::array<std::string_view, 23> stop_set = {
    "comment", "stop", "expect",    "tokens",   "separators", "ignore",
    "param",   "list", "line",      "find",     "skip",       "index",
    "repeat",  "each", "instances", "sections", "if",         "let",
    "check",   "type", "layout",    "offset",   "until"};

/// The words the type dispatch takes before a range
constexpr std::array<std::string_view, 6> type_words = {
    "int", "float", "num", "word", "rest", "alphabet"};

// The Specification's ten precedence levels, loosest first.
constexpr int disjunction = 1;
constexpr int conjunction = 2;
constexpr int equality = 3;
constexpr int relation = 4;
constexpr int additive = 5;
constexpr int multiplicative = 6;
constexpr int unary_level = 7;
constexpr int power = 8;
constexpr int postfix = 9;
constexpr int primary = 10;

std::pair<int, std::string_view> binary_level(BinaryOp op) {
  switch (op) {
  case BinaryOp::Or:
    return {disjunction, " || "};
  case BinaryOp::And:
    return {conjunction, " && "};
  case BinaryOp::Equal:
    return {equality, " == "};
  case BinaryOp::NotEqual:
    return {equality, " != "};
  case BinaryOp::Less:
    return {relation, " < "};
  case BinaryOp::LessEqual:
    return {relation, " <= "};
  case BinaryOp::Greater:
    return {relation, " > "};
  case BinaryOp::GreaterEqual:
    return {relation, " >= "};
  case BinaryOp::Add:
    return {additive, " + "};
  case BinaryOp::Subtract:
    return {additive, " - "};
  case BinaryOp::Multiply:
    return {multiplicative, " * "};
  case BinaryOp::Divide:
    return {multiplicative, " / "};
  case BinaryOp::Remainder:
    return {multiplicative, " % "};
  case BinaryOp::Power:
    return {power, " ^ "};
  }
  return {power, " ^ "};
}

int level(const Op &op) {
  switch (op.kind) {
  case OpKind::Int:
    return op.integer < 0 &&
                   op.integer != std::numeric_limits<std::int64_t>::min()
               ? unary_level
               : primary;
  case OpKind::Float:
    return std::signbit(op.number) ? unary_level : primary;
  case OpKind::Unary:
    return unary_level;
  case OpKind::Binary:
    return binary_level(static_cast<BinaryOp>(op.code)).first;
  case OpKind::Index:
  case OpKind::Member:
  case OpKind::ListMember:
    return postfix;
  default:
    return primary;
  }
}

std::size_t operand_count(const Op &op) {
  switch (op.kind) {
  case OpKind::Unary:
  case OpKind::Member:
  case OpKind::ListMember:
    return 1;
  case OpKind::Binary:
  case OpKind::Index:
  case OpKind::Comprehension:
    return 2;
  case OpKind::Call:
    return static_cast<std::size_t>(op.integer);
  default:
    return 0;
  }
}

/// Where each subexpression starts, indexed by the operation that ends it
std::vector<std::size_t> extents(const Ops &ops) {
  std::vector<std::size_t> starts;
  starts.reserve(ops.size());
  std::vector<std::size_t> stack;
  for (std::size_t index = 0; index < ops.size(); index++) {
    const std::size_t first = stack.size() - operand_count(ops[index]);
    const std::size_t start = first < stack.size() ? stack[first] : index;
    stack.resize(first);
    stack.push_back(start);
    starts.push_back(start);
  }
  return starts;
}

/// The last operation of each of the `count` operands before `end`, in order
std::vector<std::size_t> operands(const std::vector<std::size_t> &starts,
                                  std::size_t end, std::size_t n) {
  std::vector<std::size_t> ends(n);
  std::size_t next = end;
  for (std::size_t i = n; i > 0; i--) {
    next--;
    ends[i - 1] = next;
    next = starts[next];
  }
  return ends;
}

struct Task {
  enum class Kind : std::uint8_t { Expression, Text, Member, ListMember };
  Kind kind = Kind::Expression;
  std::size_t end = 0;
  int minimum = 0;
  std::string_view text;
  std::size_t record = 0;
};

Task expression_task(std::size_t end, int minimum) {
  return Task{.kind = Task::Kind::Expression,
              .end = end,
              .minimum = minimum,
              .text = {},
              .record = 0};
}

Task text_task(std::string_view text) {
  return Task{.kind = Task::Kind::Text,
              .end = 0,
              .minimum = 0,
              .text = text,
              .record = 0};
}

void write_string(std::string &out, std::string_view text) {
  out.push_back('"');
  for (const char c : text) {
    switch (c) {
    case '\\':
      out += "\\\\";
      break;
    case '"':
      out += "\\\"";
      break;
    case '\n':
      out += "\\n";
      break;
    case '\t':
      out += "\\t";
      break;
    default:
      out.push_back(c);
      break;
    }
  }
  out.push_back('"');
}

bool word_char(char c) {
  return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
         (c >= '0' && c <= '9') || c == '_';
}

std::string_view first_word(std::string_view text) {
  std::size_t end = 0;
  while (end < text.size() && word_char(text[end]))
    end++;
  return text.substr(0, end);
}

bool contains(std::span<const std::string_view> words,
              std::string_view candidate) {
  return std::ranges::find(words, candidate) != words.end();
}

/// Whether a format name can be written without quotes: names and integers
/// glued with `-` or `.`, starting with a name
bool is_bare_format(std::string_view name) {
  std::string_view remaining = name;
  bool first = true;
  while (true) {
    const std::size_t end =
        std::min(remaining.find_first_of("-."), remaining.size());
    const std::string_view part = remaining.substr(0, end);
    const std::string_view tail = remaining.substr(end);
    const bool digits = !part.empty() && std::ranges::all_of(part, [](char c) {
      return c >= '0' && c <= '9';
    });
    bool integer = false;
    if (digits) {
      std::int64_t value = 0;
      const auto [pointer, error] =
          std::from_chars(part.data(), part.data() + part.size(), value);
      integer = error == std::errc() && pointer == part.data() + part.size();
    }
    const bool valid =
        is_identifier(part) || (!first && integer && !tail.starts_with('.'));
    if (!valid)
      return false;
    if (tail.empty())
      return true;
    remaining = tail.substr(1);
    first = false;
  }
}

/// The expression of a statement that has one, such as `check` or `if`
const Ops &stmt_ops(const Stmt &stmt) {
  static const Ops none;
  return stmt.ops ? *stmt.ops : none;
}

/// Visit every expression the text of `stmt` writes: those it mentions,
/// and the types of the captures its pattern names, which print where they
/// bind
template <class F>
void printed_expressions(const State &state, const Stmt &stmt, const F &visit) {
  const auto ops_only = [&](const Mention &mention) {
    if (mention.kind == Mention::Kind::Expression)
      visit(*mention.ops);
  };
  mentions(stmt, ops_only);
  if (stmt.kind != StmtKind::Line && stmt.kind != StmtKind::Find &&
      stmt.kind != StmtKind::Arm)
    return;
  for (const Piece &piece : stmt.pattern) {
    if (piece.kind != Piece::Kind::Capture)
      continue;
    const auto found = state.names.find(piece.name);
    if (found == state.names.end() ||
        found->second.kind != NameState::Kind::Capture)
      continue;
    if (const std::optional<Declared> &spec = found->second.spec)
      mention_declared(*spec, ops_only);
  }
}

/// Internal names, chosen after the whole stream is known
struct Names {
  std::unordered_map<std::size_t, std::string> each;
  std::unordered_map<std::size_t, std::string> list;
  std::unordered_map<std::uint64_t, std::string> bindings;
};

class Fresh {
public:
  explicit Fresh(std::unordered_set<std::string> taken)
      : taken_(std::move(taken)) {}

  std::string name(std::string_view prefix) {
    std::size_t &next = next_.try_emplace(std::string(prefix), 1).first->second;
    while (true) {
      std::string name = std::string(prefix) + "_" + std::to_string(next);
      next++;
      if (taken_.insert(name).second)
        return name;
    }
  }

private:
  std::unordered_set<std::string> taken_;
  std::unordered_map<std::string, std::size_t> next_;
};

void name_bindings(Names &names, const Ops &ops, Fresh &fresh) {
  for (const Op &op : ops)
    if (op.kind == OpKind::Comprehension && op.labelled)
      names.bindings[op.id] = std::string(op.label);
  for (const Op &op : ops)
    if (op.kind == OpKind::Comprehension || op.kind == OpKind::Binding)
      if (!names.bindings.contains(op.id))
        names.bindings[op.id] = fresh.name("element");
}

/// Choose names in stream order, skipping every name in the spec, so the
/// same calls always give the same text
Names assign(const State &state) {
  std::unordered_set<std::size_t> needs_each;
  std::unordered_set<std::size_t> needs_list;
  std::unordered_set<std::string> taken;
  for (const auto &entry : state.names)
    taken.emplace(entry.first);
  for (const std::string_view type : state.types)
    taken.emplace(type);
  for (const RecordInfo &record : state.records) {
    if (record.each)
      taken.emplace(*record.each);
    if (record.list)
      taken.emplace(*record.list);
  }
  for (const Stmt &stmt : state.stmts)
    printed_expressions(state, stmt, [&](const Ops &ops) {
      for (const Op &op : ops) {
        if (op.kind == OpKind::Record)
          needs_each.insert(state.record_id(op.id));
        else if (op.kind == OpKind::RecordList || op.kind == OpKind::ListMember)
          needs_list.insert(state.record_id(op.id));
        else if (op.kind == OpKind::Comprehension && op.labelled)
          taken.emplace(op.label);
      }
    });

  Fresh fresh(std::move(taken));
  Names names;
  for (const Stmt &stmt : state.stmts) {
    if (stmt.kind == StmtKind::Repeat) {
      const std::size_t id = state.record_id(stmt.record);
      const RecordInfo &info = state.records[id];
      if (!info.each && needs_each.contains(id))
        names.each[id] = fresh.name("record");
      if (!info.list && needs_list.contains(id))
        names.list[id] = fresh.name("records");
    }
    printed_expressions(
        state, stmt, [&](const Ops &ops) { name_bindings(names, ops, fresh); });
  }
  return names;
}

bool ends_open(const Stmt &stmt) {
  if (stmt.kind != StmtKind::Declaration &&
      stmt.kind != StmtKind::ElseDeclaration &&
      stmt.kind != StmtKind::TypeDefinition)
    return false;
  const Declared &declared = stmt.declared;
  return !declared.layout && !declared.offset &&
         declared.type.kind == TypeSpec::Kind::Range && !declared.type.upper;
}

class Printer {
public:
  explicit Printer(const State &state) : state_(state), names_(assign(state)) {}

  std::expected<Printed, Issue> run() {
    statements();
    if (issue_)
      return std::unexpected(std::move(*issue_));
    return Printed{std::move(out_), std::move(sites_)};
  }

private:
  void unprintable(std::string message) {
    if (!issue_)
      issue_ = Issue{BuildErrorKind::Unprintable, std::move(message),
                     state_.sites[current_]};
  }

  void site(std::size_t index) {
    sites_.push_back(
        Printed::Entry{out_.size(), state_.sites[index],
                       state_.stmts[index].kind == StmtKind::Close});
  }

  void start_line() {
    for (std::size_t i = 0; i < indent_; i++)
      out_ += "  ";
  }

  void statements() {
    const std::vector<Stmt> &stmts = state_.stmts;
    std::size_t index = 0;
    bool continuing = false;
    while (index < stmts.size()) {
      const Stmt &stmt = stmts[index];
      current_ = index;
      site(index);
      index++;
      if (stmt.kind == StmtKind::Close) {
        indent_--;
        start_line();
        out_.push_back('}');
        continuing = after_close(index);
        continue;
      }
      if (!continuing)
        start_line();
      continuing = false;
      if (stmt.kind == StmtKind::Format && index >= 2 &&
          ends_open(stmts[index - 2]))
        unprintable("a range with no upper bound cannot come directly before "
                    "`format`: `format` would be its upper bound");
      statement(stmt);
      if (!stmt.opens_block()) {
        out_.push_back('\n');
        continue;
      }
      const std::optional<std::string> each = each_line(stmt);
      if (!each && index < stmts.size() &&
          stmts[index].kind == StmtKind::Close) {
        out_ += " {";
        site(index);
        out_.push_back('}');
        index++;
        continuing = after_close(index);
        continue;
      }
      out_ += " {\n";
      indent_++;
      if (each) {
        start_line();
        out_ += "each ";
        out_ += *each;
        out_.push_back('\n');
      }
    }
  }

  /// End the line after `}`, unless an `else` continues it
  bool after_close(std::size_t next) {
    const bool continues =
        next < state_.stmts.size() && state_.stmts[next].continues_if();
    out_.push_back(continues ? ' ' : '\n');
    return continues;
  }

  [[nodiscard]] std::optional<std::string_view>
  record_each(std::size_t record) const {
    const std::size_t id = state_.record_id(record);
    if (state_.records[id].each)
      return state_.records[id].each;
    const auto found = names_.each.find(id);
    if (found == names_.each.end())
      return std::nullopt;
    return found->second;
  }

  [[nodiscard]] std::optional<std::string_view>
  record_list(std::size_t record) const {
    const std::size_t id = state_.record_id(record);
    if (state_.records[id].list)
      return state_.records[id].list;
    const auto found = names_.list.find(id);
    if (found == names_.list.end())
      return std::nullopt;
    return found->second;
  }

  [[nodiscard]] std::optional<std::string> each_line(const Stmt &stmt) const {
    if (stmt.kind != StmtKind::Repeat)
      return std::nullopt;
    const std::optional<std::string_view> each = record_each(stmt.record);
    if (!each)
      return std::nullopt;
    return std::string(*each);
  }

  void statement(const Stmt &stmt) {
    switch (stmt.kind) {
    case StmtKind::Format:
      out_ += "format ";
      if (is_bare_format(stmt.strings[0]))
        out_ += stmt.strings[0];
      else
        write_string(out_, stmt.strings[0]);
      break;
    case StmtKind::Comment:
      strings_statement("comment", stmt.strings);
      break;
    case StmtKind::Stop:
      strings_statement("stop", stmt.strings);
      break;
    case StmtKind::Tokens:
      strings_statement("tokens", stmt.strings);
      break;
    case StmtKind::Separators:
      strings_statement("separators", stmt.strings);
      break;
    case StmtKind::IgnoreTrailing:
      out_ += "ignore trailing";
      break;
    case StmtKind::Expect:
      strings_statement("expect", stmt.strings);
      break;
    case StmtKind::Param:
      out_ += "param ";
      target(stmt.definition);
      if (stmt.ops) {
        out_ += " = ";
        expression(stmt_ops(stmt));
      }
      break;
    case StmtKind::Declaration:
      if (stmt.definition.name == std::optional<std::string_view>("format") &&
          indent_ == 0)
        unprintable("an entry named `format` has no spelling at the top "
                    "level: use a `let` or a `list`, or another name");
      declaration(stmt.definition, stmt.declared);
      break;
    case StmtKind::ElseDeclaration:
      out_ += "else ";
      declaration(stmt.definition, stmt.declared);
      break;
    case StmtKind::List:
      list(stmt.definition, stmt.declared, stmt.end);
      break;
    case StmtKind::TypeDefinition:
      out_ += "type ";
      out_ += stmt.type_name;
      out_ += " = ";
      type_with_options(stmt.declared);
      break;
    case StmtKind::Line:
      out_ += "line ";
      pattern(stmt.pattern);
      break;
    case StmtKind::Find:
      out_ += "find ";
      pattern(stmt.pattern);
      break;
    case StmtKind::SkipLine:
      out_ += "skip line";
      break;
    case StmtKind::SkipLines:
      out_ += "skip ";
      guarded(stmt_ops(stmt), std::array<std::string_view, 1>{"line"});
      out_ += " lines";
      break;
    case StmtKind::Index:
      out_ += "index ";
      expression(stmt_ops(stmt));
      break;
    case StmtKind::Let:
      out_ += "let ";
      target(stmt.definition);
      out_ += " = ";
      expression(stmt_ops(stmt));
      break;
    case StmtKind::Check:
      out_ += "check ";
      expression(stmt_ops(stmt));
      if (stmt.message) {
        out_.push_back(' ');
        write_string(out_, *stmt.message);
      }
      break;
    case StmtKind::Repeat:
      repeat(stmt);
      break;
    case StmtKind::Sections:
      sections(stmt.clauses);
      break;
    case StmtKind::Arm:
      pattern(stmt.pattern);
      if (stmt.optional)
        out_ += " optional";
      if (stmt.last)
        out_ += " last";
      break;
    case StmtKind::If:
      out_ += "if ";
      expression(stmt_ops(stmt));
      break;
    case StmtKind::ElseIf:
      out_ += "else if ";
      expression(stmt_ops(stmt));
      break;
    case StmtKind::Else:
      out_ += "else";
      break;
    case StmtKind::Instances:
      out_ += "instances";
      break;
    case StmtKind::Close:
      break;
    }
  }

  void strings_statement(std::string_view keyword,
                         const std::vector<std::string> &strings) {
    out_ += keyword;
    write_strings(strings);
  }

  void write_strings(const std::vector<std::string> &strings) {
    for (const std::string &text : strings) {
      out_.push_back(' ');
      write_string(out_, text);
    }
  }

  void target(const Definition &definition) {
    out_ += definition.name.value_or("_");
  }

  void declaration(const Definition &definition, const Declared &declared) {
    target(definition);
    for (const Ops &dimension : declared.dimensions) {
      out_.push_back('[');
      expression(dimension);
      out_.push_back(']');
    }
    out_ += ": ";
    type_with_options(declared);
  }

  void list(const Definition &definition, const Declared &declared,
            const ListEnd &end) {
    if (declared.layout)
      unprintable("a `list` has no `layout`");
    if (!declared.dimensions.empty())
      unprintable("a `list` has no dimensions: its type is a type without "
                  "`dim`");
    out_ += "list ";
    target(definition);
    out_ += ": ";
    type_name(declared);
    out_ += " until ";
    if (end.eol)
      out_ += "eol";
    else
      guarded(end.sentinel, std::array<std::string_view, 1>{"eol"});
    options(declared);
  }

  void type_with_options(const Declared &declared) {
    type_name(declared);
    options(declared);
  }

  void options(const Declared &declared) {
    if (declared.layout) {
      out_ += " layout ";
      expression(*declared.layout);
    }
    if (declared.offset) {
      out_ += " offset ";
      expression(*declared.offset);
    }
  }

  void type_name(const Declared &declared) {
    const TypeSpec &type = declared.type;
    switch (type.kind) {
    case TypeSpec::Kind::Base:
      out_ += type.base;
      break;
    case TypeSpec::Kind::Named:
      out_ += type.named;
      break;
    case TypeSpec::Kind::Alphabet:
      out_ += "alphabet ";
      write_string(out_, type.symbols);
      for (const AlphabetOption &option : type.options)
        alphabet_option(option);
      break;
    case TypeSpec::Kind::Range:
      if (type.both_upper)
        unprintable("a `Range` has both `to` and `below`: give it one upper "
                    "bound");
      if (type.lower)
        lower_bound(*type.lower);
      out_ += type.inclusive ? ".." : "..<";
      if (type.upper)
        guarded(*type.upper, stop_set);
      break;
    }
  }

  void alphabet_option(const AlphabetOption &option) {
    switch (option.kind) {
    case AlphabetOption::Kind::Blank:
      out_ += " blank ";
      write_string(out_, option.text);
      break;
    case AlphabetOption::Kind::Strip:
      out_ += " strip ";
      write_string(out_, option.text);
      break;
    case AlphabetOption::Kind::Base:
      if (option.base == std::numeric_limits<std::int64_t>::min()) {
        unprintable("the alphabet base -9223372036854775808 has no spelling");
        break;
      }
      out_ += " base ";
      out_ += std::to_string(option.base);
      break;
    }
  }

  void lower_bound(const Ops &ops) {
    const std::string text = expression_text(ops);
    const std::string_view leading = first_word(text);
    if (contains(type_words, leading) || state_.types.contains(leading))
      out_ += "(" + text + ")";
    else
      out_ += text;
  }

  /// Write an expression in parentheses if its first word is one of
  /// `words`, which would otherwise be read as a keyword
  void guarded(const Ops &ops, std::span<const std::string_view> words) {
    const std::string text = expression_text(ops);
    if (contains(words, first_word(text)))
      out_ += "(" + text + ")";
    else
      out_ += text;
  }

  std::string expression_text(const Ops &ops) {
    const std::size_t written = out_.size();
    expression(ops);
    std::string text = out_.substr(written);
    out_.resize(written);
    return text;
  }

  void repeat(const Stmt &stmt) {
    out_ += "repeat ";
    switch (stmt.repeat) {
    case RepeatKind::Times:
      guarded(stmt_ops(stmt), std::array<std::string_view, 1>{"until"});
      break;
    case RepeatKind::UntilEof:
      out_ += "until eof";
      break;
    case RepeatKind::Until:
      out_ += "until";
      write_strings(stmt.strings);
      store(stmt.store);
      break;
    }
    if (const std::optional<std::string_view> name = record_list(stmt.record)) {
      out_ += " as ";
      out_ += *name;
    }
  }

  void store(const std::optional<Definition> &definition) {
    if (definition) {
      out_ += " store ";
      target(*definition);
    }
  }

  void sections(const std::vector<SectionClause> &clauses) {
    out_ += "sections";
    for (const SectionClause &clause : clauses) {
      if (clause.skip_other) {
        out_ += " skip other";
        continue;
      }
      out_ += " until";
      write_strings(clause.terminators);
      store(clause.store);
    }
  }

  std::optional<std::string>
  capture_type(const std::optional<Declared> &capture) {
    if (!capture)
      return std::nullopt;
    const std::size_t written = out_.size();
    type_with_options(*capture);
    std::string text = out_.substr(written);
    out_.resize(written);
    if (text.contains('}'))
      unprintable("the capture type `" + text + "` contains `}`");
    return text;
  }

  void pattern(const std::vector<Piece> &pieces) {
    std::string text;
    for (const Piece &piece : pieces) {
      switch (piece.kind) {
      case Piece::Kind::Text:
        if (piece.text.contains('{') || piece.text.contains("..."))
          unprintable("the pattern text `" + piece.text +
                      "` contains `{` or `...`, which have no literal "
                      "spelling");
        text += piece.text;
        break;
      case Piece::Kind::Capture: {
        const auto found = state_.names.find(piece.name);
        const std::optional<Declared> none;
        const std::optional<Declared> &spec =
            found != state_.names.end() &&
                    found->second.kind == NameState::Kind::Capture
                ? found->second.spec
                : none;
        named_capture(text, piece.name, spec);
        break;
      }
      case Piece::Kind::Bind:
        named_capture(text, piece.name, piece.type);
        break;
      case Piece::Kind::Reference:
        text += "{$";
        text += piece.name;
        text += "}";
        break;
      case Piece::Kind::Validate:
        named_capture(text, "", piece.type);
        break;
      case Piece::Kind::Blank:
        text += "{}";
        break;
      case Piece::Kind::Discard:
        text += "{_}";
        break;
      case Piece::Kind::DiscardAs:
        named_capture(text, "_", piece.type);
        break;
      case Piece::Kind::RestOfLine:
        text += "...";
        break;
      }
    }
    write_string(out_, text);
  }

  void named_capture(std::string &text, std::string_view name,
                     const std::optional<Declared> &capture) {
    const std::optional<std::string> type = capture_type(capture);
    text += "{";
    text += name;
    if (type) {
      text += ":";
      text += *type;
    }
    text += "}";
  }

  void literal(const Op &op) {
    switch (op.kind) {
    case OpKind::Int:
      if (op.integer == std::numeric_limits<std::int64_t>::min())
        out_ += "(-9223372036854775807 - 1)";
      else
        out_ += std::to_string(op.integer);
      break;
    case OpKind::Float:
      if (std::isfinite(op.number))
        out_ += iris::detail::float_text(op.number);
      else
        unprintable(std::string("the float ") +
                    (std::isnan(op.number) ? "NaN"
                     : op.number < 0       ? "-inf"
                                           : "inf") +
                    " has no spelling");
      break;
    default:
      write_string(out_, *op.text);
      break;
    }
  }

  /// Write `.name` after the operand just written. `gecode.x` is one host
  /// constant token, so after a whole name `gecode` the dot keeps its
  /// spaces.
  void member(std::string_view name) {
    const std::string_view host_word =
        host_prefix.substr(0, host_prefix.size() - 1);
    bool whole = false;
    if (out_.ends_with(host_word)) {
      const std::size_t before = out_.size() - host_word.size();
      whole = before == 0 || !word_char(out_[before - 1]);
    }
    out_ += whole ? " . " : ".";
    out_ += name;
  }

  void expression(const Ops &ops) {
    const std::vector<std::size_t> starts = extents(ops);
    std::vector<Task> tasks{expression_task(ops.size() - 1, disjunction)};
    while (!tasks.empty()) {
      const Task task = tasks.back();
      tasks.pop_back();
      switch (task.kind) {
      case Task::Kind::Text:
        out_ += task.text;
        break;
      case Task::Kind::Member:
        member(task.text);
        break;
      case Task::Kind::ListMember:
        member(record_list(task.record).value_or(std::string_view()));
        break;
      case Task::Kind::Expression:
        if (level(ops[task.end]) < task.minimum) {
          out_.push_back('(');
          tasks.push_back(text_task(")"));
        }
        operation(ops, starts, task.end, tasks);
        break;
      }
    }
  }

  void operation(const Ops &ops, const std::vector<std::size_t> &starts,
                 std::size_t end, std::vector<Task> &tasks) {
    const Op &op = ops[end];
    switch (op.kind) {
    case OpKind::Int:
    case OpKind::Float:
    case OpKind::Str:
      literal(op);
      break;
    case OpKind::Host:
      out_ += host_prefix;
      out_ += op.name;
      break;
    case OpKind::Name:
      out_ += op.name;
      break;
    case OpKind::Binding:
      out_ += names_.bindings.at(op.id);
      break;
    case OpKind::Record:
      out_ += record_each(op.id).value_or(std::string_view());
      break;
    case OpKind::RecordList:
      out_ += record_list(op.id).value_or(std::string_view());
      break;
    case OpKind::Unary:
      out_.push_back(static_cast<UnaryOp>(op.code) == UnaryOp::Minus ? '-'
                                                                     : '!');
      tasks.push_back(expression_task(end - 1, unary_level));
      break;
    case OpKind::Binary: {
      const std::vector<std::size_t> both = operands(starts, end, 2);
      const auto binary = static_cast<BinaryOp>(op.code);
      const auto [operator_level, text] = binary_level(binary);
      const bool is_power = binary == BinaryOp::Power;
      tasks.push_back(expression_task(both[1], is_power ? unary_level
                                                        : operator_level + 1));
      tasks.push_back(text_task(text));
      tasks.push_back(
          expression_task(both[0], is_power ? postfix : operator_level));
      break;
    }
    case OpKind::Index: {
      const std::vector<std::size_t> both = operands(starts, end, 2);
      tasks.push_back(text_task("]"));
      tasks.push_back(expression_task(both[1], disjunction));
      tasks.push_back(text_task("["));
      tasks.push_back(expression_task(both[0], postfix));
      break;
    }
    case OpKind::Member:
      tasks.push_back(Task{.kind = Task::Kind::Member,
                           .end = 0,
                           .minimum = 0,
                           .text = op.name,
                           .record = 0});
      member_base(ops, end - 1, tasks);
      break;
    case OpKind::ListMember:
      tasks.push_back(Task{.kind = Task::Kind::ListMember,
                           .end = 0,
                           .minimum = 0,
                           .text = {},
                           .record = op.id});
      member_base(ops, end - 1, tasks);
      break;
    case OpKind::Call: {
      out_ += op.name;
      out_.push_back('(');
      tasks.push_back(text_task(")"));
      const std::vector<std::size_t> arguments =
          operands(starts, end, static_cast<std::size_t>(op.integer));
      for (std::size_t i = arguments.size(); i > 0; i--) {
        tasks.push_back(expression_task(arguments[i - 1], disjunction));
        if (i > 1)
          tasks.push_back(text_task(", "));
      }
      break;
    }
    case OpKind::Comprehension: {
      const std::vector<std::size_t> both = operands(starts, end, 2);
      const std::string &name = names_.bindings.at(op.id);
      if (!is_identifier(name))
        unprintable("`" + name + "` is not an Iris name");
      out_ += op.name;
      out_ += "(";
      out_ += name;
      out_ += " in ";
      tasks.push_back(text_task(")"));
      tasks.push_back(expression_task(both[1], disjunction));
      tasks.push_back(text_task(": "));
      tasks.push_back(expression_task(both[0], disjunction));
      break;
    }
    }
  }

  /// A number before `.` would lex as a malformed number, so it takes
  /// parentheses at any level
  static void member_base(const Ops &ops, std::size_t base,
                          std::vector<Task> &tasks) {
    const bool number =
        ops[base].kind == OpKind::Int || ops[base].kind == OpKind::Float;
    tasks.push_back(expression_task(base, number ? primary + 1 : postfix));
  }

  const State &state_;
  Names names_;
  std::string out_;
  std::vector<Printed::Entry> sites_;
  /// The index of the statement being printed
  std::size_t current_ = 0;
  std::size_t indent_ = 0;
  std::optional<Issue> issue_;
};

} // namespace

std::expected<Printed, Issue> print(const State &state) {
  return Printer(state).run();
}

} // namespace iris::build::detail
