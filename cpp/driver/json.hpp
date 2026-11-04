#pragma once

#include <iris/error.hpp>

#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace iris::driver {

/// A request that breaks the driver protocol, such as an unknown option
class Misuse : public std::runtime_error {
public:
  using std::runtime_error::runtime_error;
};

/// Parameters as name and value pairs, in the order given
using ParamPairs = std::vector<std::pair<std::string, std::string>>;

/// Decode the --params value: one JSON object with unique string keys and
/// string values. Throws Misuse for anything else.
ParamPairs decode_params(std::string_view json);

/// Return `text` as a JSON string. Throws std::runtime_error if `text` is not
/// valid UTF-8.
std::string quote_json(std::string_view text);

/// Return the canonical error document for `error`, ending with a newline
/// (Specification §3.7)
std::string encode_error(const Error &error);

} // namespace iris::driver
