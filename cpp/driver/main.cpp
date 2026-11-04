// The conformance driver of the C++ library: it answers the requests of the
// conformance runner (conformance/README.md) for parse, read, catalog, and
// dzn.

#include "json.hpp"

#include <iris/spec.hpp>

#include <csignal>
#include <exception>
#include <expected>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#ifdef _WIN32
#include <cstdio>
#include <fcntl.h>
#include <io.h>
#endif

namespace {

using iris::driver::Misuse;

enum class Operation { Parse, Read, Catalog, Dzn };

/// How successful data is written
enum class Output { Json, Dzn };

struct Request {
  Operation operation = Operation::Parse;
  Output output = Output::Json;
  std::string spec_path;
  std::string input_path;
  iris::driver::ParamPairs params;
};

struct Response {
  std::string output;
  int status = 0;
};

/// Exit statuses of the driver protocol
constexpr int language_error_status = 1;
constexpr int misuse_status = 2;
constexpr int host_failure_status = 3;

Request parse_request(int argc, char **argv);
std::string read_binary_file(const std::string &path);

Response execute_request(const Request &request) {
  const std::string spec_text = read_binary_file(request.spec_path);
  const auto spec = iris::Spec::try_parse(spec_text, request.spec_path);
  if (!spec) {
    std::cerr << spec.error().what() << '\n';
    return {.output = iris::driver::encode_error(spec.error()),
            .status = language_error_status};
  }
  if (request.operation == Operation::Parse)
    return {.output = "{\"iris\":\"0.1\",\"parsed\":true}\n", .status = 0};
  iris::Params params;
  for (const auto &[name, value] : request.params)
    params.set(name, value);
  const std::string input = read_binary_file(request.input_path);
  const std::expected<iris::Data, iris::Error> data =
      (request.operation == Operation::Catalog)
          ? spec->try_catalog_text(input, request.input_path, params)
          : spec->try_read_text(input, request.input_path, params);
  if (!data) {
    std::cerr << data.error().what() << '\n';
    return {.output = iris::driver::encode_error(data.error()),
            .status = language_error_status};
  }
  if (request.output == Output::Dzn)
    return {.output = std::string(data->dzn()), .status = 0};
  return {.output = iris::driver::encode_data(*data), .status = 0};
}

Operation parse_operation(std::string_view command) {
  if (command == "parse")
    return Operation::Parse;
  if (command == "read")
    return Operation::Read;
  if (command == "catalog")
    return Operation::Catalog;
  if (command == "dzn")
    return Operation::Dzn;
  throw Misuse("unknown command: " + std::string(command));
}

Request parse_request(int argc, char **argv) {
  if (argc < 3)
    throw Misuse("usage: iris-driver parse|read|catalog|dzn SPEC [INPUT] "
                 "[--params JSON] [--version 0.1] [--format json|dzn]");
  const std::vector<std::string_view> args(argv + 1, argv + argc);
  Request request;
  request.operation = parse_operation(args[0]);
  if (request.operation == Operation::Dzn)
    request.output = Output::Dzn;
  request.spec_path = args[1];
  std::size_t next = 2;
  if (request.operation != Operation::Parse) {
    if ((args.size() < 3) || args[2].starts_with("--"))
      throw Misuse("missing input path");
    request.input_path = args[next++];
  }
  if (request.spec_path.starts_with("--"))
    throw Misuse("missing spec path");
  std::set<std::string_view> seen;
  while (next < args.size()) {
    const std::string_view option = args[next++];
    if ((option != "--version") && (option != "--params") &&
        (option != "--format"))
      throw Misuse("unknown option or extra positional argument: " +
                   std::string(option));
    if (!seen.insert(option).second)
      throw Misuse("repeated option: " + std::string(option));
    if (next == args.size())
      throw Misuse("missing option value: " + std::string(option));
    const std::string_view value = args[next++];
    if (option == "--version") {
      if (value != "0.1")
        throw Misuse("unsupported Iris version: " + std::string(value));
    } else if (option == "--params") {
      request.params = iris::driver::decode_params(value);
    } else if ((request.operation != Operation::Catalog) ||
               ((value != "json") && (value != "dzn"))) {
      throw Misuse("--format json|dzn is only valid for catalog");
    } else {
      request.output = (value == "dzn") ? Output::Dzn : Output::Json;
    }
  }
  if ((request.operation == Operation::Parse) && !request.params.empty())
    throw Misuse("parse requires empty params");
  return request;
}

std::string read_binary_file(const std::string &path) {
  if (!std::filesystem::is_regular_file(path))
    throw std::runtime_error("not a regular file: " + path);
  std::ifstream file(path, std::ios::binary);
  if (!file)
    throw std::runtime_error("cannot open file: " + path);
  std::string bytes{std::istreambuf_iterator<char>(file), {}};
  if (file.bad())
    throw std::runtime_error("cannot read file: " + path);
  return bytes;
}

} // namespace

int main(int argc, char **argv) {
#ifdef SIGPIPE
  // Let failed pipe writes reach the host-failure boundary.
  std::signal(SIGPIPE, SIG_IGN);
#endif
#ifdef _WIN32
  // Write dzn and JSON bytes unchanged, without CR before LF.
  _setmode(_fileno(stdout), _O_BINARY);
#endif
  try {
    const Response response = execute_request(parse_request(argc, argv));
    std::cout << response.output;
    std::cout.flush();
    if (!std::cout)
      throw std::runtime_error("cannot write stdout");
    return response.status;
  } catch (const Misuse &error) {
    std::cerr << "iris-driver: " << error.what() << '\n';
    return misuse_status;
  } catch (const std::exception &error) {
    std::cerr << "iris-driver: " << error.what() << '\n';
    return host_failure_status;
  }
}
