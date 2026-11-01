#include "errors.hh"
#include "json.hh"
#include <csignal>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <optional>
#include <set>
#include <string>

namespace {
namespace instance = Gecode::Instance;
enum class Operation { parse, read, catalog, dzn };
enum class Output { json, dzn };
struct Request {
  Operation operation;
  Output output;
  std::string spec_path;
  std::string input_path;
  iris::ParamPairs params;
};
struct Response {
  std::string output;
  int status;
};
Request parse_request(int argc, char **argv);
std::string read_binary_file(const std::string &path);

Response language_error(iris::Phase phase, const instance::Error &error,
                        const Request &request, const std::string &input) {
  const std::string original = error.what();
  std::cerr << original << '\n';
  return {iris::encode_error(iris::decode_error(
              phase, original, request.spec_path, request.input_path, input)),
          1};
}

Response execute_request(const Request &request) {
  const auto spec_text = read_binary_file(request.spec_path);
  std::optional<instance::Spec> spec;
  try {
    spec = instance::Spec::parse(spec_text, request.spec_path);
  } catch (const instance::Error &error) {
    return language_error(iris::Phase::parse, error, request, "");
  }
  if (request.operation == Operation::parse)
    return {"{\"iris\":\"0.1\",\"parsed\":true}\n", 0};
  const auto input = read_binary_file(request.input_path);
  instance::Params params;
  for (const auto &pair : request.params)
    params.set(pair.first, pair.second);
  instance::Data data;
  try {
    data = request.operation == Operation::catalog
               ? spec->catalog_text(input, request.input_path, params)
               : spec->read_text(input, request.input_path, params);
  } catch (const instance::Error &error) {
    return language_error(iris::Phase::read, error, request, input);
  }
  return {request.output == Output::dzn ? iris::render_dzn(data)
                                        : iris::encode_data(data),
          0};
}

Request parse_request(int argc, char **argv) {
  if (argc < 3)
    throw iris::Misuse(
        "usage: iris-gecode-driver parse|read|catalog|dzn SPEC [INPUT] "
        "[--params JSON] [--version 0.1] [--format json|dzn]");
  const std::string operation = argv[1];
  Request request{};
  if (operation == "parse")
    request.operation = Operation::parse;
  else if (operation == "read")
    request.operation = Operation::read;
  else if (operation == "catalog")
    request.operation = Operation::catalog;
  else if (operation == "dzn")
    request.operation = Operation::dzn;
  else
    throw iris::Misuse("unknown command: " + operation);
  request.output =
      request.operation == Operation::dzn ? Output::dzn : Output::json;
  request.spec_path = argv[2];
  int next = 3;
  if (request.operation != Operation::parse) {
    if (argc < 4 || std::string_view(argv[3]).substr(0, 2) == "--")
      throw iris::Misuse("missing input path");
    request.input_path = argv[next++];
  }
  if (std::string_view(request.spec_path).substr(0, 2) == "--")
    throw iris::Misuse("missing spec path");
  std::set<std::string> seen;
  while (next < argc) {
    const std::string option = argv[next++];
    if (option != "--version" && option != "--params" && option != "--format")
      throw iris::Misuse("unknown option or extra positional argument: " +
                         option);
    if (!seen.insert(option).second)
      throw iris::Misuse("repeated option: " + option);
    if (next == argc)
      throw iris::Misuse("missing option value: " + option);
    const std::string value = argv[next++];
    if (option == "--version") {
      if (value != "0.1")
        throw iris::Misuse("unsupported Iris version: " + value);
    } else if (option == "--params") {
      request.params = iris::decode_params(value);
    } else {
      if (request.operation != Operation::catalog ||
          (value != "json" && value != "dzn"))
        throw iris::Misuse("--format json|dzn is only valid for catalog");
      request.output = value == "dzn" ? Output::dzn : Output::json;
    }
  }
  if (request.operation == Operation::parse && !request.params.empty())
    throw iris::Misuse("parse requires empty params");
  return request;
}

std::string read_binary_file(const std::string &path) {
  if (!std::filesystem::is_regular_file(path))
    throw std::runtime_error("not a regular file: " + path);
  std::ifstream file(path, std::ios::binary);
  if (!file)
    throw std::runtime_error("cannot open file: " + path);
  const std::string bytes{std::istreambuf_iterator<char>(file), {}};
  if (file.bad())
    throw std::runtime_error("cannot read file: " + path);
  return bytes;
}
} // namespace

int main(int argc, char **argv) {
#ifdef SIGPIPE
  // Let failed pipe writes reach the host-output failure boundary.
  std::signal(SIGPIPE, SIG_IGN);
#endif
  try {
    const auto response = execute_request(parse_request(argc, argv));
    std::cout << response.output;
    std::cout.flush();
    if (!std::cout)
      throw std::runtime_error("cannot write stdout");
    return response.status;
  } catch (const iris::Misuse &error) {
    std::cerr << "iris-gecode-driver: " << error.what() << '\n';
    return 2;
  } catch (const std::exception &error) {
    std::cerr << "iris-gecode-driver: " << error.what() << '\n';
    return 3;
  }
}
