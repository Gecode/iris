#pragma once

#include <iris/build.hpp>

#include <cstdint>
#include <string>
#include <string_view>
#include <tuple>

/// A job shop reader and the entries a scheduler needs
struct Jobshop {
  iris::Spec spec;
  iris::Field<std::string, 0, iris::Presence::Maybe> name;
  iris::Field<std::int64_t> n_jobs;
  iris::Field<std::int64_t> n_machines;
  iris::Field<std::int64_t, 2> machine;
  iris::Field<std::int64_t, 2> duration;
};

/// Reads OR-Library and JSPLIB files: a header line with the numbers of jobs
/// and machines, then one line per job of machine and duration pairs
inline Jobshop jobshop() {
  using namespace iris::build;
  constexpr std::string_view machine_twice =
      "a job uses a machine twice; each job must use every machine once";
  constexpr std::string_view too_long = "the total duration is too large";

  SpecBuilder b("jobshop");
  b.comment({"#", "+++"});
  auto handles = b.instances([&](auto &i) {
    auto name = i.optional_capture("name", word);
    auto n_jobs = i.capture("n_jobs", Range{.from = 1});
    auto n_machines = i.capture("n_machines", Range{.from = 1});
    auto records = i.sections({.skip_other = true}, [&](auto &s) {
      s.optional_arm(pattern("instance {}", name));
      auto header = pattern("{} {}", n_jobs, n_machines);
      return s.arm(header, {.last = true}, [&](auto &a) {
        a.check(n_jobs <= (host::int_max - 2) / 2 / n_machines,
                "there are too many jobs and machines");
        return a.repeat(n_jobs, [&](auto &job) {
          job.as_name("jobs"); // error messages name the records
          auto steps = job.repeat(n_machines, [&](auto &step) {
            return std::tuple{
                step.read("machine", Range{.from = 0, .below = n_machines}),
                step.read("duration", Range{.from = 0})};
          });
          return std::tuple_cat(std::tuple{job.record()}, steps);
        });
      });
    });
    auto [jobs, machine, duration] = records;
    i.check(distinct(jobs.get(machine)), machine_twice);
    auto total = sum(jobs, [&](const auto &j) { return sum(j.get(duration)); });
    i.check(total <= host::int_max, too_long);
    return std::tuple{name, n_jobs, n_machines, machine, duration};
  });
  auto [name, n_jobs, n_machines, machine, duration] = handles;
  return {b.build(), name, n_jobs, n_machines, machine, duration};
}
