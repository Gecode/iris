#pragma once

#include <cstddef>
#include <cstdint>

namespace iris::detail {

/**
 * The work budgets of one read (Specification chapter 10)
 *
 * The reader charges idle work, such as repeat iterations and empty rows
 * that read no input, and the evaluator charges expression effort, at the
 * sites where Gecode counts them. The limits and the exact accounting of
 * chapter 10 are not implemented yet, so the charges have no effect: task
 * cpp-003 implements them.
 */
class Budget {
public:
  /// The budgets for an input of `input_bytes` bytes
  explicit Budget(std::size_t /*input_bytes*/) {}
  /// Charge `units` of idle work (§10.2)
  void charge_idle(std::uint64_t /*units*/) {}
  /// Charge `units` of expression effort (§10.3)
  void charge_effort(std::uint64_t /*units*/) {}
};

} // namespace iris::detail
