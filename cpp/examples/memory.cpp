#include <iris/iris.hpp>

#include <cstddef>
#include <memory_resource>
#include <print>

// Passes every allocation on to the default resource, and counts the bytes
// that are allocated and not yet returned.
class CountingResource : public std::pmr::memory_resource {
public:
  [[nodiscard]] std::size_t held() const { return held_; }

private:
  void *do_allocate(std::size_t bytes, std::size_t alignment) override {
    held_ += bytes;
    return upstream_->allocate(bytes, alignment);
  }
  void do_deallocate(void *pointer, std::size_t bytes,
                     std::size_t alignment) override {
    held_ -= bytes;
    upstream_->deallocate(pointer, bytes, alignment);
  }
  [[nodiscard]] bool
  do_is_equal(const std::pmr::memory_resource &other) const noexcept override {
    return this == &other;
  }

  std::pmr::memory_resource *upstream_ = std::pmr::get_default_resource();
  std::size_t held_ = 0;
};

int main() {
  CountingResource memory;
  {
    const iris::Spec spec =
        iris::Spec::parse("n:int row[n]:int", "example.iris", memory);
    const iris::Data data = spec.read_text("3 10 20 30", "input.txt");
    std::println("{} {}", data.integer("n"), memory.held() > 0);
  }
  std::println("{}", memory.held());
}
