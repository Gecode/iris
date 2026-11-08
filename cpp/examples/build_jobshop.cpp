#include "build_jobshop.hpp"

#include <iris/iris.hpp>

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <print>
#include <span>
#include <string>

namespace {

/// `row` as `[1, 2, 3]`
std::string list(std::span<const std::int64_t> row) {
  std::string text = "[";
  for (std::size_t i = 0; i < row.size(); ++i)
    text += (i == 0 ? "" : ", ") + std::to_string(row[i]);
  return text + "]";
}

} // namespace

int main() {
  const Jobshop shop = jobshop();
  iris::Params params;
  params.set("instance", "handmade-or");
  const iris::Data data =
      shop.spec.read("formats/jobshop/cases/or-library/input.txt", params);
  std::println("{}: {} jobs on {} machines",
               data.get(shop.name).value_or("unnamed"), data.get(shop.n_jobs),
               data.get(shop.n_machines));
  const auto machines = data.get(shop.machine);
  const auto durations = data.get(shop.duration);
  for (std::size_t job = 0; job < machines.size(); ++job) {
    std::println("job {}: machines {}, durations {}", job, list(machines[job]),
                 list(durations[job]));
  }

  params.set("instance", "handmade-invalid");
  try {
    (void)shop.spec.read("formats/jobshop/cases/duplicate-machine/input.txt",
                         params);
  } catch (const iris::Error &error) {
    const auto &at = error.input_location();
    std::println("\n{} at {}:{}:{}", iris::code_name(error.code()), at->origin,
                 at->line.value(), at->column.value());
    std::println("{}", error.message());
    // The statement's call site: the line of the distinct check in jobshop().
    const auto &site = error.call_site();
    const std::filesystem::path file = site->file_name();
    std::println("call site: {}:{}", file.filename().string(), site->line());
  }
}
