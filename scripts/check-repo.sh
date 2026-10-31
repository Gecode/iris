#!/usr/bin/env bash
set -euo pipefail

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
failed=0

# Build the capitalized name in parts so this script passes its own name check.
capitalized_name=IR'IS'
# Exceptions are exact repository-relative paths followed by a colon and an
# extended regular expression matching the allowed line. One per line; empty today.
name_exceptions=''

check_required_files() {
  local file
  for file in README.md LICENSE AGENTS.md CONTEXT.md PLAN.md docs/adr/README.md; do
    if [[ ! -f "$file" ]]; then
      printf '%s: required file is missing\n' "$file" >&2
      failed=1
    fi
  done
}

check_name() {
  local matches status=0 file line content exception allowed
  matches=$(git -c core.quotePath=false grep -I -nw -- "$capitalized_name") || status=$?
  if [[ $status -gt 1 ]]; then
    printf 'Repository name check: git grep failed (exit %s)\n' "$status" >&2
    failed=1
    return
  fi
  [[ -n "$matches" ]] || return 0
  while IFS=: read -r file line content; do
    allowed=0
    while IFS= read -r exception; do
      [[ -n "$exception" ]] || continue
      if [[ "$file" == "${exception%%:*}" && "$content" =~ ${exception#*:} ]]; then
        allowed=1
        break
      fi
    done <<< "$name_exceptions"
    if [[ $allowed -eq 0 ]]; then
      printf '%s:%s: use Iris or iris, never the name in capitals\n' "$file" "$line" >&2
      failed=1
    fi
  done <<< "$matches"
}

# Emit line<TAB>path for inline links/images and reference definitions.
# Definitions cover full, collapsed, and shortcut references without resolving
# labels. See README.md for the link conventions this parser checks.
markdown_paths() {
  LC_ALL=C awk '
    # CommonMark subset: inline links/images, top-level reference definitions,
    # paragraph-local code spans, and fences in quotes and space-indented lists.
    # HTML blocks, indented code blocks, tab-indented containers, quotes inside
    # lists, and reference definitions inside containers are unsupported.

    function code_span_end(text, start,    size, i, run) {
      size = 1
      while (substr(text, start + size, 1) == "`") size++
      for (i = start + size; i <= length(text); i += run) {
        if (substr(text, i, 1) != "`") { run = 1; continue }
        run = 1
        while (substr(text, i + run, 1) == "`") run++
        if (run == size) return i + run - 1
      }
      return 0
    }

    # Set link_path and return the first position after its destination.
    function destination_end(text, start,    i, char, angle, depth) {
      link_path = ""
      while (substr(text, start, 1) ~ /[[:space:]]/) start++
      angle = substr(text, start, 1) == "<"
      for (i = start + angle; i <= length(text); i++) {
        char = substr(text, i, 1)
        if (char == "\\" && substr(text, i + 1, 1) ~ /[[:punct:]]/) {
          link_path = link_path substr(text, ++i, 1)
          continue
        }
        if (angle) {
          if (char == ">") return i + 1
          if (char == "\n" || char == "<") return 0
        } else {
          if (char ~ /[[:space:]]/ || (char == ")" && depth == 0))
            return depth == 0 ? i : 0
          if (char == "<") return 0
          if (char == "(") depth++
          if (char == ")") depth--
        }
        link_path = link_path char
      }
      return !angle && depth == 0 ? i : 0
    }

    function inline_link_end(text, start,    i, separator, quote, char) {
      i = destination_end(text, start)
      if (!i) return 0
      separator = substr(text, i, 1) ~ /[[:space:]]/
      while (substr(text, i, 1) ~ /[[:space:]]/) i++
      if (substr(text, i, 1) == ")") return i
      quote = substr(text, i, 1)
      if (!separator || (quote != "\"" && quote != sprintf("%c", 39) && quote != "("))
        return 0
      if (quote == "(") quote = ")"
      for (i++; i <= length(text); i++) {
        char = substr(text, i, 1)
        if (char == "\\" && substr(text, i + 1, 1) ~ /[[:punct:]]/) { i++; continue }
        if (char == quote) break
      }
      for (i++; substr(text, i, 1) ~ /[[:space:]]/; i++) {}
      return substr(text, i, 1) == ")" ? i : 0
    }

    function scan_paragraph(    i, end, char, brackets, line, skipped) {
      line = paragraph_line
      for (i = 1; i <= length(paragraph); i++) {
        char = substr(paragraph, i, 1)
        if (char == "\n") line++
        if (char == "\\" && substr(paragraph, i + 1, 1) ~ /[[:punct:]]/) { i++; continue }
        end = 0
        if (char == "`") {
          end = code_span_end(paragraph, i)
          if (!end) {
            while (substr(paragraph, i + 1, 1) == "`") i++
          }
        }
        if (char == "[") brackets++
        if (char == "]" && brackets > 0) {
          brackets--
          if (substr(paragraph, i + 1, 1) == "(") {
            end = inline_link_end(paragraph, i + 2)
            if (end) emit_path(line, link_path)
          }
        }
        if (end) {
          skipped = substr(paragraph, i, end - i + 1)
          line += gsub(/\n/, "", skipped)
          i = end
        }
      }
      paragraph = ""
    }

    function strip_containers(text,    indent, base, prefix) {
      quote_depth = 0
      while ((fence_char == "" || quote_depth < fence_quote_depth) \
          && match(text, /^ *> ?/)) {
        prefix = substr(text, 1, RLENGTH)
        if (index(prefix, ">") > 4) break
        text = substr(text, RLENGTH + 1)
        quote_depth++
      }
      if (quote_depth != previous_quote_depth) list_depth = 0
      new_item = 0
      if (text ~ /^ *$/) return text
      match(text, /^ */)
      indent = RLENGTH
      while (list_depth && indent < list_indent[list_depth]) list_depth--
      base = list_depth ? list_indent[list_depth] : 0
      text = substr(text, base + 1)
      # Fence contents cannot open further containers.
      if (fence_char != "" && quote_depth == fence_quote_depth \
          && list_depth >= fence_list_depth) return text
      while (match(text, /^ *([-+*]|[0-9]+[.)]) +/)) {
        prefix = substr(text, 1, RLENGTH)
        match(prefix, /^ */)
        if (RLENGTH > 3) break
        base += length(prefix)
        list_indent[++list_depth] = base
        text = substr(text, length(prefix) + 1)
        new_item = 1
      }
      return text
    }

    function consume_fence(text,    marker, rest, indent) {
      if (!match(text, /^ *(```+|~~~+)/)) return 0
      marker = substr(text, 1, RLENGTH)
      rest = substr(text, RLENGTH + 1)
      sub(/^ */, "", marker)
      indent = RLENGTH - length(marker)
      if (indent > 3) return 0
      if (fence_char == "") {
        if (substr(marker, 1, 1) == "`" && index(rest, "`")) return 0
        fence_char = substr(marker, 1, 1)
        fence_size = length(marker)
        fence_quote_depth = quote_depth
        fence_list_depth = list_depth
        return 1
      }
      if (substr(marker, 1, 1) != fence_char || length(marker) < fence_size \
          || rest !~ /^[[:space:]]*$/) return 0
      fence_char = ""
      return 1
    }

    function emit_path(line, path,    i, hex, decoded, digits) {
      if (path ~ /^([[:alpha:]][[:alnum:]+.-]*:|\/|#)/) return
      sub(/[?#].*$/, "", path)
      if (path == "") return
      digits = "0123456789abcdef"
      for (i = 1; i <= length(path); i++) {
        hex = tolower(substr(path, i + 1, 2))
        if (substr(path, i, 1) == "%" && hex ~ /^[[:xdigit:]][[:xdigit:]]$/) {
          decoded = decoded sprintf("%c", (index(digits, substr(hex, 1, 1)) - 1) * 16 \
            + index(digits, substr(hex, 2, 1)) - 1)
          i += 2
        } else decoded = decoded substr(path, i, 1)
      }
      printf "%d\t%s\n", line, decoded
    }

    {
      content = strip_containers($0)
      container_changed = quote_depth != previous_quote_depth \
        || list_depth != previous_list_depth || new_item
      previous_quote_depth = quote_depth
      previous_list_depth = list_depth
      if (fence_char != "" && (quote_depth != fence_quote_depth \
          || list_depth < fence_list_depth || new_item)) fence_char = ""
      if (consume_fence(content)) { scan_paragraph(); next }
      if (fence_char != "") next
      block_line = content ~ /^ *#+([[:space:]]|$)/ \
        || content ~ /^ *([-*_] *)+$/ || content ~ /^ *===* *$/
      if (container_changed || block_line || content ~ /^[[:space:]]*$/) scan_paragraph()

      if (definition_line) {
        if (destination_end(content, 1)) emit_path(definition_line, link_path)
        definition_line = 0
        next
      }
      if (quote_depth == 0 && list_depth == 0 && paragraph ~ /^[[:space:]]*$/ \
          && match(content, /^ *\[[^]]+\]:[[:space:]]*/)) {
        definition = substr(content, 1, RLENGTH)
        target = substr(content, RLENGTH + 1)
        if (index(definition, "[") <= 4) {
          scan_paragraph()
          if (target == "") definition_line = NR
          else if (destination_end(target, 1)) emit_path(NR, link_path)
          next
        }
      }
      if (paragraph == "") paragraph_line = NR
      paragraph = paragraph content "\n"
      if (block_line) scan_paragraph()
    }

    END { scan_paragraph() }
  ' "$1"
}

check_markdown_links() {
  local file line path paths
  while IFS= read -r -d '' file; do
    if [[ ! -f "$file" ]]; then
      printf '%s: tracked Markdown file is missing\n' "$file" >&2
      failed=1
      continue
    fi
    if ! paths=$(markdown_paths "$file"); then
      printf '%s: cannot parse Markdown links\n' "$file" >&2
      failed=1
      continue
    fi
    [[ -n "$paths" ]] || continue
    while IFS=$'\t' read -r line path; do
      if [[ ! -e "$(dirname "$file")/$path" ]]; then
        printf '%s:%s: broken relative Markdown link: %s\n' "$file" "$line" "$path" >&2
        failed=1
      fi
    done <<< "$paths"
  done < <(git ls-files -z -- '*.md')
}

check_required_files
check_name
check_markdown_links
if [[ $failed -ne 0 ]]; then
  exit 1
fi
printf 'Repository checks passed.\n'
