#pragma once

#include <iris/error.hpp>
#include <iris/spec.hpp>

#include <expected>
#include <functional>
#include <string>

namespace iris::driver {

/// Gives the spec that answers a request whose spec file is at `path`
using SpecLoader =
    std::function<std::expected<Spec, Error>(const std::string &path)>;

/// Return the bytes of the regular file at `path`. Throws
/// std::runtime_error if it cannot be read.
std::string read_binary_file(const std::string &path);

/// Parse the reader spec in the file at `path`, which names it in errors
std::expected<Spec, Error> parse_spec_file(const std::string &path);

/// Answer the request of the conformance runner in `argv` with the spec that
/// `load` gives, and return the protocol's exit status
int run(int argc, char **argv, const SpecLoader &load);

} // namespace iris::driver
