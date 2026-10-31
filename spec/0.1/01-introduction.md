# 1 Introduction

## 1.1 Scope and version

The Iris Language Specification defines Iris 0.1: the language in which a
reader spec describes how to read a text instance into named, typed data.
It defines reader-spec syntax, input reading, the data model, errors, work
budgets, and dzn output. The [chapter index](README.md) identifies the parts
still being written.

Iris 0.1 is the language of Gecode 6.5.0, subject to the deliberate
[differences in appendix C](appendix-c-differences.md). Iris 1.0 develops
the language further for Gecode 7. Language versions and package versions
are separate: each package MUST state the language versions it supports.

## 1.2 Reference behavior

While the Gecode `release-6.5.0` tag is unavailable, the reference is the
Gecode commit `5e7bbd5c26b0702c6fd652356b2289da0710ac3c` on branch
`feature/6.5.0`. The repository's [reference build script](../../scripts/README.md#build-the-gecode-reference)
builds this pin independently of any other Gecode checkout.

The pinned implementation's behavior takes precedence over its Doxygen
commentary when they disagree, unless appendix C records a deliberate
difference. Probes and source inspection establish the behavior described
here. The pinned Gecode reader is the oracle while the Specification is
written; Rust is the project's reference implementation.

Iris adds stable error codes and canonical JSON for comparisons between
implementations. Gecode's messages are mapped to those codes by its
conformance driver; Gecode itself need not emit them. Iris also requires
UTF-8 input, reader specs, and Unicode strings: the whole input, the whole
reader-spec source, and each `word` or `rest` value actually read MUST be
valid UTF-8, as specified in [§4.10](04-input.md#410-utf-8-validity),
[§5.1](05-reader-specs.md#51-reader-specs-and-parsing), and appendix C.

## 1.3 Conformance

An implementation conforms to Iris 0.1 when it passes the 0.1 conformance
suite and follows every applicable MUST and MUST NOT rule in the
Specification. The suite is the common contract between implementations;
it does not replace the normative text. The project terms used here are
defined in [CONTEXT.md](../../CONTEXT.md).

Conformance checks ordered entries, their value types and depths, values,
and the compared error fields defined in [§3.8](03-data-model.md#38-error-comparison).
Examples use the extractable format in [chapter 2](02-notation.md).
Message wording is recommended and is not a conformance input.

Once released, the 0.1 Specification is frozen. Corrections are errata;
new language features belong to Iris 1.0.
