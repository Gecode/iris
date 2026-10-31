# spec

This directory holds the Iris Language Specification, the normative document
for each language version. The [0.1 chapter index](0.1/README.md) separates
completed chapters from planned chapters, the formal grammar, and the error
code registry. `1.0/` follows once the 1.0 design starts.

The Draft 2020-12 schemas for [data](schema/data.schema.json) and
[errors](schema/error.schema.json) check the canonical JSON encodings. Their
[semantic limits](0.1/03-data-model.md#39-schema-validation) are part of the
encoding contract. A conformance-case schema is planned.

[PLAN.md §6](../PLAN.md#6-the-iris-01-specification) describes this component;
milestone M1 completes the 0.1 Specification.
