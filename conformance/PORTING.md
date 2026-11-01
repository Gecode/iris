# Porting Gecode tests

This convention belongs to the conformance suite in [PLAN.md §7](../PLAN.md#7-conformance-suite),
milestone M2. Use the [case and driver contract](README.md) for every ported case.
The reference is Gecode commit `5e7bbd5c26b0702c6fd652356b2289da0710ac3c`.

1. Read `test/instance.cpp` at that pin with
   `git -C /Users/zayenz/gecode/gecode show 5e7bbd5c26b0702c6fd652356b2289da0710ac3c:test/instance.cpp`.
   Never change or build that checkout. Record the pin and assigned source lines.
2. Enumerate every source test group and distinct execution, including loop
   iterations, helper calls, and trailing fails/rejects returns outside `CHECK`.
   Several accessor assertions on one successful execution share one case with
   the complete output.
3. Preserve original reader-spec/input bytes and string parameters. Group cases
   by language chapter and name them `<test-name>-<n>`. Record
   `source = "gecode:test/instance.cpp:<TestName>"` in case.toml. Set the operation
   and catalog output where required.
4. Obtain complete success output from the adapter. Review every expressible
   original assertion against it; retain all entries, types, depths, order and
   values. Review errors against the registry, original expected rejection,
   and primary location/path. `Data::Output` assertions become dzn cases with
   exact bytes. Split different operations and outcomes into separate cases.
5. List each host/API-only assertion individually by source line or assertion
   identity, with its concrete exclusion reason. Do not exclude a group because
   one accessor or narrowing assertion is unportable. Full int64 reads remain
   portable even where `IntArgs` narrowing is not.
6. Map every group's case directories and individual exclusions below. Run the
   batch and report counts and how all retained assertions were checked. Do not
   rewrite expectations to silence disagreements. Stop and report an unapproved
   Specification/reference disagreement. An accepted appendix C difference may
   use only the precise [known-failure contract](README.md#known-failures).

A group may occupy several rows. Every exclusion needs its own row; there is no
implicit unported remainder. Porting batches add their mappings independently.
Specification examples are extracted separately under the
[example transport convention](README.md#specification-examples-and-later-extraction).

| Source group / scenario or assertion | Pinned source lines | Case directories | Host/API exclusion and reason |
|---|---|---|---|
