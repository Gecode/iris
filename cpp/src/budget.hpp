#pragma once

#include <cstddef>
#include <cstdint>

namespace iris::detail {

/**
 * The work budgets of one read or catalog (Specification chapter 10)
 *
 * The reader charges idle work (§10.2): repeat iterations that read no
 * input and the rows of arrays with an empty dimension. The evaluator
 * charges expression effort (§10.3, §10.4). For an input of N bytes, idle
 * work may reach B = 2^20 units and expression effort B + 16 * N units
 * (§10.1). A charge fails exactly when its counter would exceed the limit;
 * reaching the limit is allowed.
 *
 * The accounting is exact (§10.1, appendix C.3): each budget keeps the units
 * still allowed, which can exceed 2^64 for the largest N, so nothing
 * overflows. One budget spans the whole operation, prefix and instances
 * alike (§10.5); the caller reports exhaustion (§10.6).
 */
class Budget {
public:
  /// The budgets for an input of `input_bytes` bytes
  explicit Budget(std::size_t input_bytes)
      : idle_(0, base_limit), effort_(effort_limit(input_bytes)) {}
  /// Charge `units` of idle work, and return whether they are allowed
  [[nodiscard]] bool charge_idle(std::uint64_t units) {
    return idle_.spend(units);
  }
  /// Charge `units` of expression effort, and return whether they are
  /// allowed
  [[nodiscard]] bool charge_effort(std::uint64_t units) {
    return effort_.spend(units);
  }

private:
  /// The units still allowed: high * 2^64 + low
  class Allowance {
  public:
    Allowance(std::uint64_t high, std::uint64_t low) : high_(high), low_(low) {}
    /// Take `units`, and return whether there were that many
    [[nodiscard]] bool spend(std::uint64_t units) {
      if (low_ >= units) {
        low_ -= units;
        return true;
      }
      if (high_ == 0)
        return false;
      // Borrow 2^64: the difference wraps to the exact remainder
      high_--;
      low_ -= units;
      return true;
    }

  private:
    std::uint64_t high_;
    std::uint64_t low_;
  };

  /// B, the idle limit and the part of the effort limit without input
  static constexpr std::uint64_t base_limit = std::uint64_t{1} << 20;
  /// Return B + 16 * n, the effort limit for an input of `n` bytes
  static Allowance effort_limit(std::uint64_t n) {
    const std::uint64_t scaled = n << 4;
    const std::uint64_t low = scaled + base_limit;
    return {(n >> 60) + std::uint64_t{low < scaled}, low};
  }

  Allowance idle_;
  Allowance effort_;
};

} // namespace iris::detail
