#ifndef iris_gecode_errors_hh
#define iris_gecode_errors_hh

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace iris {
enum class Phase { parse, read };
struct Location {
  std::string origin;
  std::optional<std::size_t> line;
  std::optional<std::size_t> column;
};
struct Diagnostic {
  std::string code;
  std::string message;
  std::optional<Location> input_location;
  std::optional<Location> spec_location;
  std::optional<std::string> context;
  std::optional<std::vector<std::size_t>> record_path;
};
struct Pattern {
  Phase phase;
  std::string code;
  std::string text;
};
const std::vector<Pattern> &patterns();
std::string expand_pattern(const Pattern &pattern);
std::string map_message(Phase phase, const std::string &message);
Diagnostic decode_error(Phase phase, const std::string &diagnostic,
                        const std::string &spec_origin,
                        const std::string &input_origin,
                        const std::string &input);
} // namespace iris
#endif
