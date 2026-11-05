#pragma once

#include <array>
#include <charconv>
#include <concepts>
#include <cstddef>
#include <iterator>
#include <limits>
#include <memory>
#include <memory_resource>
#include <new>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

namespace iris::detail {

/*
 * The allocation hook
 *
 * Every allocation the library makes comes from the memory resource of the
 * operation that makes it: the Spec's for parsing and reading, the
 * Params' for setting parameters, the Data's for rendering it. Each entry
 * point holds a MemoryScope for that resource, and the containers of the
 * library use Allocator, which takes the innermost scope's resource of the
 * thread when it is created and keeps it. So containers built during an
 * operation allocate from its resource, and they free their memory there
 * wherever they are destroyed, and threads that read at the same time each use
 * their own scope.
 *
 * Errors are the exception: an Error, its strings, and the exceptions the
 * library throws use the global heap, as they leave the library and
 * outlive the operation.
 */

/// Return the resource an Allocator keeps for `memory`: null for
/// std::pmr::new_delete_resource(), which Allocator replaces by
/// std::allocator, as compilers can elide its allocations and not those
/// through a resource
inline std::pmr::memory_resource *kept(std::pmr::memory_resource &memory) {
  return (&memory == std::pmr::new_delete_resource()) ? nullptr : &memory;
}

/// The resource of the innermost scope of this thread as Allocator keeps it
/// (see kept), or null outside every scope
inline thread_local std::pmr::memory_resource *scope_memory = nullptr;

/// Make the library allocate from `memory` on this thread while this
/// object exists
class MemoryScope {
public:
  explicit MemoryScope(std::pmr::memory_resource &memory)
      : outer_(std::exchange(scope_memory, kept(memory))) {}
  ~MemoryScope() { scope_memory = outer_; }
  MemoryScope(const MemoryScope &) = delete;
  MemoryScope(MemoryScope &&) = delete;
  MemoryScope &operator=(const MemoryScope &) = delete;
  MemoryScope &operator=(MemoryScope &&) = delete;

private:
  std::pmr::memory_resource *outer_;
};

/**
 * Allocator of the library's containers
 *
 * It allocates from the resource of the innermost scope when it was
 * created, or from the global heap outside every scope. A container that
 * is moved from passes its allocator on, so that moving never allocates.
 * Like std::pmr::polymorphic_allocator, it does not propagate on copy
 * assignment or swap, and a copied container takes the current resource.
 */
template <class T> class Allocator {
public:
  // The names the standard gives allocators
  using value_type = T; // NOLINT(readability-identifier-naming)
  // Moved containers keep their memory, so that moves do not allocate
  using propagate_on_container_move_assignment = // NOLINT(readability-identifier-naming)
      std::true_type;

  Allocator() noexcept : memory_(scope_memory) {}
  /// Allocate from `memory`
  explicit Allocator(std::pmr::memory_resource &memory) noexcept
      : memory_(kept(memory)) {}
  template <class U>
  Allocator(const Allocator<U> &other) noexcept // NOLINT: like std::allocator
      : memory_(kept(other.memory())) {}

  [[nodiscard]] T *allocate(std::size_t n) {
    if (memory_ == nullptr)
      return std::allocator<T>().allocate(n);
    if (n > (std::numeric_limits<std::size_t>::max() / sizeof(T)))
      throw std::bad_array_new_length();
    return static_cast<T *>(memory_->allocate(n * sizeof(T), alignof(T)));
  }
  void deallocate(T *p, std::size_t n) noexcept {
    if (memory_ == nullptr)
      std::allocator<T>().deallocate(p, n);
    else
      memory_->deallocate(static_cast<void *>(p), n * sizeof(T), alignof(T));
  }
  [[nodiscard]] Allocator select_on_container_copy_construction() const {
    return {};
  }
  /// Return the resource this allocates from
  [[nodiscard]] std::pmr::memory_resource &memory() const {
    return (memory_ != nullptr) ? *memory_ : *std::pmr::new_delete_resource();
  }

  template <class U>
  friend bool operator==(const Allocator &a, const Allocator<U> &b) noexcept {
    return (&a.memory() == &b.memory()) || a.memory().is_equal(b.memory());
  }

private:
  /// The resource, or null for the global heap
  std::pmr::memory_resource *memory_;
};

/// Growable array
template <class T> using Seq = std::vector<T, Allocator<T>>;

/// String of the library
using String = std::basic_string<char, std::char_traits<char>, Allocator<char>>;

/// Return the decimal spelling of `v`
template <std::integral T> String to_string(T v) {
  // Enough for the digits and sign of every 64-bit integer
  std::array<char, 24> digits{};
  char *const first = digits.data();
  char *const end = std::to_chars(first, first + digits.size(), v).ptr;
  return {first, end};
}

/// Destroys and frees a Node with the allocator it was allocated with
template <class T> class NodeDelete {
public:
  NodeDelete() = default;
  explicit NodeDelete(std::pmr::memory_resource &memory) : memory_(&memory) {}
  void operator()(T *p) const noexcept {
    std::destroy_at(p);
    Allocator<T>(*memory_).deallocate(p, 1);
  }

private:
  std::pmr::memory_resource *memory_ = nullptr;
};

/// Owner of a syntax-tree node
template <class T> using Node = std::unique_ptr<T, NodeDelete<T>>;

/// Return a new syntax-tree node, constructed from `args`
///
/// Every node is allocated here.
template <class T, class... Args> Node<T> make_node(Args &&...args) {
  Allocator<T> allocator;
  T *p = allocator.allocate(1);
  try {
    std::construct_at(p, std::forward<Args>(args)...);
  } catch (...) {
    allocator.deallocate(p, 1);
    throw;
  }
  return Node<T>(p, NodeDelete<T>(allocator.memory()));
}

/// Return a new shared `T`, constructed from `args`
template <class T, class... Args>
std::shared_ptr<T> make_shared(Args &&...args) {
  return std::allocate_shared<T>(Allocator<T>(), std::forward<Args>(args)...);
}

} // namespace iris::detail
