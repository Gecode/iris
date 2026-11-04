// Tests of the exact limits of the work budgets (Specification §10.1), at
// sizes that the conformance suite cannot reach.

#include "budget.hpp"

#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <limits>
#include <source_location>
#include <string_view>

namespace {

int failures = 0;

void check(bool condition, std::string_view what,
           std::source_location where = std::source_location::current()) {
  if (condition)
    return;
  failures++;
  std::cerr << where.file_name() << ':' << where.line()
            << ": check failed: " << what << '\n';
}

constexpr std::uint64_t base_limit = std::uint64_t{1} << 20;

void allows_the_limits_without_input() {
  iris::detail::Budget budget(0);
  check(budget.charge_idle(base_limit - 1), "idle below the limit");
  check(budget.charge_idle(1), "idle at the limit");
  check(!budget.charge_idle(1), "idle over the limit");
  check(budget.charge_effort(base_limit), "effort at the limit");
  check(!budget.charge_effort(1), "effort over the limit");
}

void counts_input_bytes() {
  iris::detail::Budget budget(2);
  check(!budget.charge_effort(base_limit + 33), "one unit over B + 32");
  iris::detail::Budget exact(2);
  check(exact.charge_effort(base_limit + 32), "B + 32 for two bytes");
  check(exact.charge_idle(base_limit), "idle limit without bytes");
}

// The effort limit B + 16 * SIZE_MAX exceeds 2^64
void counts_past_two_to_the_64() {
  if constexpr (std::numeric_limits<std::size_t>::max() ==
                std::numeric_limits<std::uint64_t>::max()) {
    constexpr std::uint64_t most = std::numeric_limits<std::uint64_t>::max();
    iris::detail::Budget budget(std::numeric_limits<std::size_t>::max());
    // 16 * (2^64 - 1) units, leaving B
    for (int i = 0; i < 16; i++)
      check(budget.charge_effort(most), "a charge of 2^64 - 1");
    check(budget.charge_effort(base_limit), "the rest of the limit");
    check(!budget.charge_effort(1), "one unit over the limit");
    iris::detail::Budget whole(std::numeric_limits<std::size_t>::max());
    check(!whole.charge_idle(base_limit + 1), "idle ignores the input");
  }
}

} // namespace

int main() {
  allows_the_limits_without_input();
  counts_input_bytes();
  counts_past_two_to_the_64();
  if (failures > 0) {
    std::cerr << failures << " checks failed\n";
    return EXIT_FAILURE;
  }
  std::cout << "all checks passed\n";
  return EXIT_SUCCESS;
}
