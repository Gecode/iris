#include <iris/build.hpp>

#include <exception>
#include <iostream>

namespace {

using namespace iris::build;

SpecBuilder builder() {
  SpecBuilder b("jobshop");
  auto name = b.forward<DynField>("name");
  auto n_jobs = b.forward<DynField>("n_jobs");
  auto n_machines = b.forward<DynField>("n_machines");
  auto machine = b.forward<DynField>("machine");
  auto duration = b.forward<DynField>("duration");
  auto job = b.forward_record("job", "jobs");
  b.comment({"#", "+++"});
  b.instances([&](auto &s1) {
    s1.sections({.skip_other = true}, [&](auto &a2) {
      a2.optional_arm(pattern("instance {}", bind(name, word)));
      auto e1 = pattern("{} {}", bind(n_jobs, Range{.from = 1}),
                        bind(n_machines, Range{.from = 1}));
      a2.arm(e1, {.last = true}, [&](auto &s3) {
        s3.check(n_jobs <= (host::int_max - 2) / 2 / n_machines,
                 "there are too many jobs and machines");
        s3.repeat(n_jobs, [&](auto &s4) {
          s4.record_as(job);
          s4.repeat(n_machines, [&](auto &s5) {
            s5.each_name("step");
            s5.as_name("steps");
            s5.read(machine, Range{.from = 0, .below = n_machines});
            s5.read(duration, Range{.from = 0});
          });
        });
      });
    });
    s1.check(
        distinct(job.get(machine)),
        "a job uses a machine twice; each job must use every machine once");
    auto e2 = sum(over(job.list()).named("j"),
                  [&](const auto &j) { return sum(j.get(duration)); });
    s1.check(e2 <= host::int_max, "the total duration is too large");
  });
  return b;
}

} // namespace

int main() {
  try {
    std::cout << builder().source();
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}
