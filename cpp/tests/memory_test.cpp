// Tests of the allocation hook: given a memory resource, the library
// allocates from it and from nothing else. The test replaces the global
// operator new to count the allocations that bypass the resource.

#include <iris/iris.hpp>

#include <atomic>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <memory_resource>
#include <new>
#include <source_location>
#include <span>
#include <string>
#include <string_view>
#include <system_error>
#include <thread>

#ifdef _WIN32
#include <crtdbg.h>
#include <malloc.h>
#endif

namespace {

/// Return `bytes` of memory aligned to `alignment` from the C heap, or null
void *heap_allocate(std::size_t bytes, std::size_t alignment) {
  if (alignment <= alignof(std::max_align_t))
    return std::malloc(bytes == 0 ? 1 : bytes);
#ifdef _WIN32
  return _aligned_malloc(bytes, alignment);
#else
  // aligned_alloc needs a size that is a multiple of the alignment
  return std::aligned_alloc(alignment,
                            (bytes + alignment - 1) / alignment * alignment);
#endif
}

/// Free `p`, which heap_allocate returned for `alignment`
void heap_free(void *p, std::size_t alignment) noexcept {
#ifdef _WIN32
  if (alignment > alignof(std::max_align_t)) {
    _aligned_free(p);
    return;
  }
#else
  static_cast<void>(alignment);
#endif
  std::free(p);
}

/// Whether the global operator new counts as a forbidden allocation on
/// this thread
thread_local bool global_heap_forbidden = false;
/// The calls of the global operator new while it was forbidden
std::atomic<std::size_t> forbidden_allocations{0};

/// Forbid the global operator new on this thread while this object exists
class ForbidGlobalHeap {
public:
  ForbidGlobalHeap() { global_heap_forbidden = true; }
  ~ForbidGlobalHeap() { global_heap_forbidden = false; }
  ForbidGlobalHeap(const ForbidGlobalHeap &) = delete;
  ForbidGlobalHeap(ForbidGlobalHeap &&) = delete;
  ForbidGlobalHeap &operator=(const ForbidGlobalHeap &) = delete;
  ForbidGlobalHeap &operator=(ForbidGlobalHeap &&) = delete;
};

/// Allocate from the global heap, counting the call if it is forbidden
void *global_allocate(std::size_t bytes, std::size_t alignment) {
  if (global_heap_forbidden)
    forbidden_allocations++;
  void *p = heap_allocate(bytes, alignment);
  if (p == nullptr)
    throw std::bad_alloc();
  return p;
}

/// A thread-safe resource on the C heap that counts its allocations and
/// the bytes it has handed out and not got back
class CountingResource : public std::pmr::memory_resource {
public:
  [[nodiscard]] std::size_t allocations() const { return allocations_; }
  [[nodiscard]] std::size_t outstanding() const { return outstanding_; }

private:
  void *do_allocate(std::size_t bytes, std::size_t alignment) override {
    void *p = heap_allocate(bytes, alignment);
    if (p == nullptr)
      throw std::bad_alloc();
    allocations_++;
    outstanding_ += bytes;
    return p;
  }
  void do_deallocate(void *p, std::size_t bytes,
                     std::size_t alignment) override {
    outstanding_ -= bytes;
    heap_free(p, alignment);
  }
  [[nodiscard]] bool
  do_is_equal(const std::pmr::memory_resource &other) const noexcept override {
    return this == &other;
  }

  std::atomic<std::size_t> allocations_{0};
  std::atomic<std::size_t> outstanding_{0};
};

int failures = 0;

void check(bool condition, std::string_view what,
           std::source_location where = std::source_location::current()) {
  if (condition)
    return;
  failures++;
  std::cerr << where.file_name() << ':' << where.line()
            << ": check failed: " << what << '\n';
}

/// The job shop format of formats/jobshop: instances, sections, nested
/// repeats with records, and checks over them
constexpr std::string_view jobshop_spec = R"(format jobshop

comment "#" "+++"
instances {
  sections skip other {
    "instance {name:word}" optional
    "{n_jobs:1..} {n_machines:1..}" last {
      check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "too many jobs"

      repeat n_jobs as jobs {
        each job
        repeat n_machines as steps {
          each step
          machine: 0..<n_machines
          duration: 0..
        }
      }
    }
  }

  check distinct(job.machine) "a job uses a machine twice"
  check sum(j in jobs: sum(j.duration)) <= gecode.int_max "too long"
}
)";

constexpr std::string_view jobshop_input = R"(+++ two instances
instance first
2 2
0 3 1 4
1 5 0 6
+++
instance second
1 3
2 1 0 1 1 1
)";

/// A spec with parameters, floats, strings, lists, layouts, and lets
constexpr std::string_view mixed_spec = R"(format mixed
param scale = 2
tokens ";"
line "size {n:1..} {label}"
w[n][n]: int layout "UPPER_ROW" offset 1
repeat n {
  list xs: float until eol
}
words[2]: word
let total = sum(w) * scale
if total > 0 {
  note: rest
} else {
  let none = 0
}
check total == 36 "the total"
)";

constexpr std::string_view mixed_input = "size 3 grid\n"
                                         "1 2 3\n"
                                         "0.5 1e3\n"
                                         "2.25\n"
                                         "-7\n"
                                         "alpha ;\n"
                                         "a free note\n";

/// Read every kind of data and render it with `memory` as the only heap
void reads_with_resource() {
  CountingResource memory;
  {
    const ForbidGlobalHeap forbid;
    const iris::Spec jobshop =
        iris::Spec::parse(jobshop_spec, "jobshop.iris", memory);
    const iris::Params second({{"instance", "second"}}, memory);
    const iris::Data data = jobshop.read_text(jobshop_input, "jobs", second);
    check(data.integer("n_machines") == 3, "selected instance");
    check(data.text("name") == "second", "instance name");
    const iris::Data catalog = jobshop.catalog_text(jobshop_input, "jobs");
    check(catalog.integer("instances") == 2, "catalog");
    const std::pmr::string dzn = data.dzn();
    check(dzn.get_allocator().resource() == &memory, "dzn allocator");
    check(!dzn.empty(), "dzn");

    const iris::Spec mixed =
        iris::Spec::parse(mixed_spec, "mixed.iris", memory);
    iris::Params scale(memory);
    scale.set("scale", "2");
    // A typed float value is formatted without leaving the resource
    iris::Params ratio(memory);
    ratio.set(iris::Field<double>("ratio"),
              -std::numeric_limits<double>::max());
    check(ratio.get("ratio") == "-1.7976931348623157e308", "typed float");
    const auto read = mixed.try_read_text(mixed_input, "mixed", scale);
    check(read.has_value(), "mixed read");
    if (read) {
      const std::span<const double> first = read->floats("xs", {0});
      check((first.size() == 2) && (first[1] == 1000.0), "floats");
      check(read->text("label") == "grid", "label");
      check(read->text("note") == "a free note", "rest");
      check(!read->dzn().empty(), "mixed dzn");
    }
  }
  check(forbidden_allocations == 0, "no global allocation");
  check(memory.allocations() > 0, "allocations from the resource");
  check(memory.outstanding() == 0, "all memory returned");
}

/// Load a spec and read files with `memory` as the only heap, apart from
/// the C library's FILE objects
void reads_files_with_resource() {
  const std::filesystem::path directory =
      std::filesystem::temp_directory_path() / "iris-memory-test";
  std::filesystem::create_directories(directory);
  const std::filesystem::path spec_path = directory / "jobshop.iris";
  const std::filesystem::path input_path = directory / "jobs.txt";
  std::ofstream(spec_path, std::ios::binary) << jobshop_spec;
  std::ofstream(input_path, std::ios::binary) << jobshop_input;
  const std::filesystem::path missing = directory / "missing.txt";
  CountingResource memory;
  {
    const ForbidGlobalHeap forbid;
    const iris::Spec spec = iris::Spec::load(spec_path, memory);
    const iris::Params first({{"instance", "1"}}, memory);
    check(spec.read(input_path, first).integer("n_jobs") == 2, "read file");
    check(spec.try_catalog(input_path)->integer("instances") == 2,
          "catalog file");
    check(spec.catalog(input_path).integer("instances") == 2,
          "throwing catalog file");
    check(iris::Spec::try_load(spec_path, memory).has_value(), "try_load");
  }
  check(forbidden_allocations == 0, "no global allocation with files");
  check(memory.outstanding() == 0, "all memory of files returned");
  bool thrown = false;
  try {
    static_cast<void>(iris::Spec::load(missing, memory));
  } catch (const std::system_error &) {
    thrown = true;
  }
  check(thrown, "a missing file throws");
  thrown = false;
  {
    const iris::Spec spec = iris::Spec::load(spec_path, memory);
    try {
      static_cast<void>(spec.catalog(missing));
    } catch (const std::system_error &) {
      thrown = true;
    }
  }
  check(thrown, "catalog of a missing file throws");
  check(memory.outstanding() == 0, "no memory kept by a missing file");
  std::filesystem::remove_all(directory);
}

/// Errors use the global heap, but take nothing from the resource for good
void returns_memory_on_errors() {
  CountingResource memory;
  {
    const auto bad_spec =
        iris::Spec::try_parse("x: int\nlet y = @", "bad", memory);
    check(!bad_spec &&
              (bad_spec.error().code() == iris::ErrorCode::SpecCharacter),
          "parse error");
    const iris::Spec spec = iris::Spec::parse(jobshop_spec, "jobshop", memory);
    const auto bad_read = spec.try_read_text("1 2\n0 1 0 1\n", "bad");
    check(!bad_read &&
              (bad_read.error().code() == iris::ErrorCode::CheckFailed),
          "read error");
    const iris::Data catalog = spec.catalog_text("1 2\n0 1 0 1\n", "bad");
    check(catalog.text("failed", {0}) == "a job uses a machine twice",
          "rejected instance");
  }
  check(memory.outstanding() == 0, "all memory returned after errors");
}

/// Without a resource, the library allocates from the default resource
void uses_default_resource() {
  CountingResource memory;
  std::pmr::memory_resource *outer = std::pmr::set_default_resource(&memory);
  {
    const ForbidGlobalHeap forbid;
    const iris::Spec spec = iris::Spec::parse(jobshop_spec);
    const iris::Params params{{"instance", "first"}};
    check(spec.read_text(jobshop_input, "jobs", params).integer("n_jobs") == 2,
          "read with the default resource");
  }
  std::pmr::set_default_resource(outer);
  check(forbidden_allocations == 0, "no global allocation by default");
  check(memory.allocations() > 0, "allocations from the default resource");
  check(memory.outstanding() == 0, "default resource memory returned");
}

/// Threads read with one spec and its resource at the same time
void reads_on_threads() {
  CountingResource memory;
  const iris::Spec spec = iris::Spec::parse(jobshop_spec, "jobshop", memory);
  const std::size_t before = memory.allocations();
  const iris::Params first({{"instance", "1"}}, memory);
  std::atomic<int> reads{0};
  const auto read = [&] {
    const ForbidGlobalHeap forbid;
    for (int k = 0; k < 50; k++)
      if (spec.read_text(jobshop_input, "jobs", first).integer("n_jobs") == 2)
        reads++;
  };
  {
    std::jthread a(read);
    std::jthread b(read);
  }
  check(reads == 100, "every read on threads");
  check(forbidden_allocations == 0, "no global allocation on threads");
  check(memory.allocations() > before, "threads allocate from the resource");
}

} // namespace

// The replacements of the global allocation functions, named as the
// standard library declares them
// NOLINTBEGIN(readability-inconsistent-declaration-parameter-name)
void *operator new(std::size_t bytes) {
  return global_allocate(bytes, alignof(std::max_align_t));
}
void *operator new(std::size_t bytes, std::align_val_t alignment) {
  return global_allocate(bytes, static_cast<std::size_t>(alignment));
}
void operator delete(void *p) noexcept {
  heap_free(p, alignof(std::max_align_t));
}
void operator delete(void *p, std::size_t) noexcept {
  heap_free(p, alignof(std::max_align_t));
}
void operator delete(void *p, std::align_val_t alignment) noexcept {
  heap_free(p, static_cast<std::size_t>(alignment));
}
void operator delete(void *p, std::size_t,
                     std::align_val_t alignment) noexcept {
  heap_free(p, static_cast<std::size_t>(alignment));
}

// NOLINTEND(readability-inconsistent-declaration-parameter-name)

/// Fails every allocation after the first `budget`
class FailingResource : public std::pmr::memory_resource {
public:
  explicit FailingResource(std::size_t budget) : budget_(budget) {}
  [[nodiscard]] std::size_t outstanding() const { return outstanding_; }

private:
  void *do_allocate(std::size_t bytes, std::size_t alignment) override {
    if (budget_ == 0)
      throw std::bad_alloc();
    budget_--;
    void *p = heap_allocate(bytes, alignment);
    if (p == nullptr)
      throw std::bad_alloc();
    outstanding_ += bytes;
    return p;
  }
  void do_deallocate(void *p, std::size_t bytes,
                     std::size_t alignment) override {
    outstanding_ -= bytes;
    heap_free(p, alignment);
  }
  [[nodiscard]] bool
  do_is_equal(const std::pmr::memory_resource &other) const noexcept override {
    return this == &other;
  }

  std::size_t budget_;
  std::size_t outstanding_ = 0;
};

/// Params::set copies arguments that point into the same Params
void sets_params_safely() {
  iris::Params aliased{{"first", "1"}};
  for (int i = 0; i < 8; i++)
    aliased.set("name" + std::to_string(i), aliased.name(0));
  check(aliased.size() == 9 && aliased.get("name7") == "first",
        "set from a name of the same Params");
  aliased.set("first", aliased.name(3));
  check(aliased.get("first") == "name2", "replace from the same Params");
}

/// Params::set leaves the Params unchanged when an allocation fails
void keeps_params_on_failure() {
  // Long strings, so that every set allocates.
  const std::string key(64, 'k');
  const std::string value(64, 'v');
  for (std::size_t budget = 0; budget < 16; budget++) {
    FailingResource memory(budget);
    {
      iris::Params params(memory);
      std::size_t expected = 0;
      for (int i = 0; i < 3; i++) {
        bool failed = false;
        try {
          params.set(key + std::to_string(i), value);
        } catch (const std::bad_alloc &) {
          failed = true;
        }
        if (!failed)
          expected++;
        check(params.size() == expected, "size after a failed set");
        for (std::size_t j = 0; j < params.size(); j++)
          check(params.get(params.name(j)) == value,
                "value after a failed set");
      }
    }
    check(memory.outstanding() == 0, "memory after a failed set");
  }
}

/// Whether moving a container allocates, as it does in a debug MSVC build. It
/// does so in a noexcept constructor, so a failing resource ends the program.
#if defined(_ITERATOR_DEBUG_LEVEL) && (_ITERATOR_DEBUG_LEVEL > 0)
constexpr bool moves_allocate = true;
#else
constexpr bool moves_allocate = false;
#endif

/// Report assertions and aborts on standard error. The debug C runtime on
/// Windows shows them in a dialog, which waits forever without a desktop.
void report_failures_on_stderr() {
#ifdef _WIN32
  for (const int kind : {_CRT_WARN, _CRT_ERROR, _CRT_ASSERT}) {
    _CrtSetReportMode(kind, _CRTDBG_MODE_FILE);
    _CrtSetReportFile(kind, _CRTDBG_FILE_STDERR);
  }
  _set_abort_behavior(0, _WRITE_ABORT_MSG | _CALL_REPORTFAULT);
#endif
}

/// Run `step`, naming it first so that a crash or a timeout shows where
void run(std::string_view name, void (*step)()) {
  std::cerr << name << '\n';
  step();
}

int main() {
  report_failures_on_stderr();
  try {
    run("reads_with_resource", reads_with_resource);
    run("reads_files_with_resource", reads_files_with_resource);
    run("returns_memory_on_errors", returns_memory_on_errors);
    run("uses_default_resource", uses_default_resource);
    run("reads_on_threads", reads_on_threads);
    run("sets_params_safely", sets_params_safely);
    if constexpr (!moves_allocate)
      run("keeps_params_on_failure", keeps_params_on_failure);
  } catch (const std::exception &error) {
    std::cerr << "unexpected exception: " << error.what() << '\n';
    return EXIT_FAILURE;
  }
  if (failures > 0) {
    std::cerr << failures << " checks failed\n";
    return EXIT_FAILURE;
  }
  std::cout << "all checks passed\n";
  return EXIT_SUCCESS;
}
