#ifndef iris_gecode_json_hh
#define iris_gecode_json_hh

#include "errors.hh"
#include <gecode/instance.hh>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace iris {
class Misuse : public std::runtime_error {
public:
  using std::runtime_error::runtime_error;
};
using ParamPairs = std::vector<std::pair<std::string, std::string>>;
ParamPairs decode_params(std::string_view json);
bool valid_utf8(std::string_view text);
std::string quote_json(std::string_view text);
std::string encode_float(double value);
std::string encode_data(const Gecode::Instance::Data &data);
std::string render_dzn(const Gecode::Instance::Data &data);
std::string encode_error(const Diagnostic &diagnostic);
} // namespace iris
#endif
