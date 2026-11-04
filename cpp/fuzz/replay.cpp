// Runs a fuzz target without libFuzzer: over each file argument, and over the
// files below each directory argument in sorted order. This keeps the targets
// compiled and tested with every compiler, and reproduces a libFuzzer
// artifact where libFuzzer is missing.

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <vector>

// NOLINTNEXTLINE(readability-identifier-naming)
extern "C" int LLVMFuzzerTestOneInput(const std::uint8_t *data,
                                      std::size_t size);

namespace {

/// Return the files named by `argument`: the file itself, or the regular
/// files below a directory, sorted
std::vector<std::filesystem::path>
input_files(const std::filesystem::path &argument) {
  if (!std::filesystem::is_directory(argument))
    return {argument};
  std::vector<std::filesystem::path> files;
  for (const auto &entry :
       std::filesystem::recursive_directory_iterator(argument))
    if (entry.is_regular_file())
      files.push_back(entry.path());
  std::ranges::sort(files);
  return files;
}

std::vector<std::uint8_t> read_bytes(const std::filesystem::path &path) {
  std::ifstream file(path, std::ios::binary);
  if (!file)
    throw std::runtime_error("cannot open file: " + path.string());
  std::vector<std::uint8_t> bytes;
  for (auto byte = std::istreambuf_iterator<char>(file);
       byte != std::istreambuf_iterator<char>(); ++byte)
    bytes.push_back(static_cast<std::uint8_t>(*byte));
  if (file.bad())
    throw std::runtime_error("cannot read file: " + path.string());
  return bytes;
}

} // namespace

int main(int argc, char **argv) {
  if (argc < 2) {
    std::cerr << "usage: " << argv[0] << " FILE_OR_DIRECTORY...\n";
    return 2;
  }
  try {
    const std::vector<std::string> arguments(argv + 1, argv + argc);
    std::size_t count = 0;
    for (const auto &argument : arguments) {
      for (const auto &path : input_files(argument)) {
        const std::vector<std::uint8_t> bytes = read_bytes(path);
        LLVMFuzzerTestOneInput(bytes.data(), bytes.size());
        ++count;
      }
    }
    std::cout << "replayed " << count << " inputs\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << argv[0] << ": " << error.what() << '\n';
    return 2;
  }
}
