#pragma once

/**
 * \mainpage C++ API reference
 *
 * The public API of the C++ library. `#include <iris/iris.hpp>` declares all
 * of it, in the namespace iris.
 *
 * - iris::Spec parses and checks a reader spec, and reads input with it.
 * - iris::Params holds the parameters of a read.
 * - iris::Data holds the entries a read produces, and iris::ValueRef views
 *   a value without copying it. iris::AccessError reports misuse of both.
 * - iris::Error is a language error, with an iris::ErrorCode, the
 *   iris::Location of the error, and iris::ErrorDetails.
 * - iris::Field is a typed handle to an entry: iris::Data::get returns the
 *   entry through it as an integer, a span, an iris::Array and so on.
 *
 * `#include <iris/build.hpp>` adds the spec builder in the namespace
 * iris::build. iris::build::SpecBuilder writes a reader spec from C++
 * statements that return handles, and iris::build::BuildError reports a
 * mistake with the call site of the statement that made it.
 */

#include <iris/data.hpp>
#include <iris/error.hpp>
#include <iris/handle.hpp>
#include <iris/params.hpp>
#include <iris/spec.hpp>

/// The C++ implementation of Iris
namespace iris {}
