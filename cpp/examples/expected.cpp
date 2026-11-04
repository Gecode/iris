#include <iris/iris.hpp>

#include <expected>
#include <print>

int main() {
  const std::expected<iris::Spec, iris::Error> rejected =
      iris::Spec::try_parse("n:int row[n]:nothing", "example.iris");
  if (!rejected) {
    std::println("{}", iris::code_name(rejected.error().code()));
  }

  const iris::Spec spec = iris::Spec::parse("n:int row[n]:int", "example.iris");
  const std::expected<iris::Data, iris::Error> data =
      spec.try_read_text("2 10 twenty", "input.txt");
  if (data) {
    std::println("{}", data->integer("n"));
  } else {
    std::println("{}", data.error().what());
  }
}
