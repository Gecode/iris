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
 */

#include <iris/data.hpp>
#include <iris/error.hpp>
#include <iris/params.hpp>
#include <iris/spec.hpp>

/// The C++ implementation of Iris
namespace iris {}
