#include "text.hpp"

namespace iris::detail {

namespace {

/// The bytes that may follow a lead byte: the range of the second byte and
/// the number of continuation bytes in all (Unicode table 3-7)
struct Sequence {
  unsigned char second_min;
  unsigned char second_max;
  std::size_t continuations;
};

/// Return the sequence that lead byte `lead` starts, or nothing if `lead`
/// cannot start a well-formed sequence of more than one byte
std::optional<Sequence> sequence_after(unsigned char lead) {
  if ((lead >= 0xc2) && (lead <= 0xdf))
    return Sequence{.second_min = 0x80, .second_max = 0xbf, .continuations = 1};
  if (lead == 0xe0)
    return Sequence{.second_min = 0xa0, .second_max = 0xbf, .continuations = 2};
  if (lead == 0xed)
    return Sequence{.second_min = 0x80, .second_max = 0x9f, .continuations = 2};
  if ((lead >= 0xe1) && (lead <= 0xef))
    return Sequence{.second_min = 0x80, .second_max = 0xbf, .continuations = 2};
  if (lead == 0xf0)
    return Sequence{.second_min = 0x90, .second_max = 0xbf, .continuations = 3};
  if ((lead >= 0xf1) && (lead <= 0xf3))
    return Sequence{.second_min = 0x80, .second_max = 0xbf, .continuations = 3};
  if (lead == 0xf4)
    return Sequence{.second_min = 0x80, .second_max = 0x8f, .continuations = 3};
  return std::nullopt;
}

/// Test whether `c` is in `lo`..`hi`
bool is_between(unsigned char c, unsigned char lo, unsigned char hi) {
  return (c >= lo) && (c <= hi);
}

} // namespace

std::optional<std::size_t> find_invalid_utf8(std::string_view text) {
  std::size_t p = 0;
  while (p < text.size()) {
    const auto lead = static_cast<unsigned char>(text[p]);
    if (lead < 0x80) {
      p++;
      continue;
    }
    const std::optional<Sequence> sequence = sequence_after(lead);
    if (!sequence || (text.size() - p <= sequence->continuations))
      return p;
    if (!is_between(static_cast<unsigned char>(text[p + 1]),
                    sequence->second_min, sequence->second_max))
      return p;
    for (std::size_t k = 2; k <= sequence->continuations; k++)
      if (!is_between(static_cast<unsigned char>(text[p + k]), 0x80, 0xbf))
        return p;
    p += sequence->continuations + 1;
  }
  return std::nullopt;
}

std::string_view first_character(std::string_view text) {
  const auto lead = static_cast<unsigned char>(text[0]);
  const std::optional<Sequence> sequence = sequence_after(lead);
  return text.substr(0, sequence ? sequence->continuations + 1 : 1);
}

} // namespace iris::detail
