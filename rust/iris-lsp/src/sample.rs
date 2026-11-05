//! Directive parsing and one immutable buffer's sample reads. No syntax nodes cross threads.
use crate::{
    document::{Document, Positions},
    formatting,
};
use gecode_iris_core::{CancellationToken, Failure, HostError, Params, ReadOptions, Spec, summary};
use gecode_iris_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, Hover, HoverContents, InlayHint, InlayHintLabel,
    InlayHintTooltip, MarkupContent, MarkupKind, NumberOrString, Uri,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(crate) struct Directive {
    pub range: TextRange,
    pub end: TextSize,
    pub label: String,
    pub path: Option<PathBuf>,
    pub params: Params,
    issues: Vec<Issue>,
}
#[derive(Clone)]
struct Issue {
    severity: DiagnosticSeverity,
    message: String,
}
#[derive(Default)]
pub(crate) struct Samples {
    pub directives: Vec<Directive>,
    pub outcomes: Vec<Outcome>,
    pub cancellation: Option<CancellationToken>,
    pub checker: Vec<Diagnostic>,
}
#[derive(Clone)]
pub(crate) enum Outcome {
    Pending,
    Success { preview: String, hint: String },
    Failed(Diagnostic),
}
impl Samples {
    pub fn cancel(&mut self) -> bool {
        if let Some(token) = self.cancellation.take() {
            token.cancel();
        }
        let had_hints = self
            .outcomes
            .iter()
            .any(|o| matches!(o, Outcome::Success { .. }));
        self.outcomes.clear();
        self.directives.clear();
        self.checker.clear();
        had_hints
    }
    pub fn diagnostics(&self, positions: &Positions) -> Vec<Diagnostic> {
        let mut diagnostics = self.checker.clone();
        for (index, directive) in self.directives.iter().enumerate() {
            diagnostics.extend(directive.issues.iter().map(|issue| Diagnostic {
                range: positions.range(directive.range),
                severity: Some(issue.severity),
                code: Some(NumberOrString::String("sample-directive".into())),
                source: Some("iris".into()),
                message: format!("{}: {}", directive.label, issue.message),
                ..Diagnostic::default()
            }));
            if let Some(Outcome::Failed(error)) = self.outcomes.get(index) {
                diagnostics.push(error.clone());
            }
        }
        diagnostics
    }
    pub fn hover(&self, offset: TextSize, positions: &Positions, markdown: bool) -> Option<Hover> {
        let (index, directive) = self
            .directives
            .iter()
            .enumerate()
            .find(|(_, d)| d.range.contains(offset))?;
        let text = match self.outcomes.get(index) {
            Some(Outcome::Success { preview, .. }) => preview.clone(),
            Some(Outcome::Failed(error)) => error.message.clone(),
            _ if directive.path.is_none() => directive
                .issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            _ => format!("Reading {} …", directive.label),
        };
        Some(Hover {
            contents: HoverContents::Markup(markup(text, markdown)),
            range: Some(positions.range(directive.range)),
        })
    }
    pub fn hints(&self, range: TextRange, positions: &Positions, markdown: bool) -> Vec<InlayHint> {
        self.directives
            .iter()
            .zip(&self.outcomes)
            .filter_map(|(directive, outcome)| {
                let Outcome::Success { preview, hint } = outcome else {
                    return None;
                };
                range.contains_inclusive(directive.end).then(|| InlayHint {
                    position: positions.at(directive.end),
                    label: InlayHintLabel::String(hint.clone()),
                    kind: None,
                    tooltip: Some(InlayHintTooltip::MarkupContent(markup(
                        preview.clone(),
                        markdown,
                    ))),
                    padding_left: Some(true),
                    padding_right: None,
                    text_edits: None,
                    data: None,
                })
            })
            .collect()
    }
}
fn markup(text: String, markdown: bool) -> MarkupContent {
    if !markdown {
        return MarkupContent {
            kind: MarkupKind::PlainText,
            value: text,
        };
    }
    let mut longest = 0;
    let mut run = 0;
    for b in text.bytes() {
        if b == b'`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(3.max(longest + 1));
    MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!("{fence}\n{text}\n{fence}"),
    }
}

#[derive(Default)]
struct ParseWork {
    #[cfg(test)]
    visits: usize,
}
impl ParseWork {
    #[inline]
    fn visit(&mut self, _count: usize) {
        #[cfg(test)]
        {
            self.visits += _count;
        }
    }
}

pub(crate) fn parse(root: SyntaxNode, uri: &Uri, positions: &Positions) -> Vec<Directive> {
    parse_counted(root, uri, positions, &mut ParseWork::default())
}
fn parse_counted(
    root: SyntaxNode,
    uri: &Uri,
    positions: &Positions,
    work: &mut ParseWork,
) -> Vec<Directive> {
    let mut directives = Vec::new();
    for element in root.descendants_with_tokens() {
        work.visit(1);
        let Some(token) = element.into_token() else {
            continue;
        };
        if token.kind() != SyntaxKind::Comment {
            continue;
        }
        work.visit(token.text().len());
        let raw = token.text().trim_end_matches(['\r', '\n']);
        let Some(text) = raw
            .strip_prefix('#')
            .map(|s| s.trim_start_matches([' ', '\t']))
            .and_then(|s| s.strip_prefix("iris:"))
        else {
            continue;
        };
        let range = TextRange::new(
            token.text_range().start(),
            token.text_range().start() + TextSize::try_from(raw.len()).expect("token length"),
        );
        let line = positions.at(range.start()).line + 1;
        directives.push(parse_line(text, range, line, uri, work));
    }
    directives
}
fn label_part(text: &str, scalars: usize) -> String {
    let mut chars = text.chars();
    let mut label: String = chars.by_ref().take(scalars).collect();
    if chars.next().is_some() {
        label.pop();
        label.push('…');
    }
    label
}

fn parse_line(
    text: &str,
    range: TextRange,
    line: u32,
    uri: &Uri,
    work: &mut ParseWork,
) -> Directive {
    let mut directive = Directive {
        range,
        end: range.end(),
        label: format!("sample (directive line {line})"),
        path: None,
        params: Params::new(),
        issues: Vec::new(),
    };
    let words = match words(text, work) {
        Ok(words) => words,
        Err(message) => {
            directive.error(message);
            return directive;
        }
    };
    if words.first().map(String::as_str) != Some("sample") {
        directive.warning("unknown directive command");
        return directive;
    }
    let Some(path) = words.get(1).filter(|p| !p.is_empty()) else {
        directive.error("sample requires a nonempty relative PATH");
        return directive;
    };
    let mut label = format!("sample {}", label_part(path, 80));
    let relative = Path::new(path);
    if relative.has_root()
        || relative
            .components()
            .any(|c| matches!(c, std::path::Component::Prefix(_)))
        || path.contains('\0')
    {
        directive.error("sample PATH must be a valid relative path");
    } else if !uri
        .scheme()
        .is_some_and(|s| s.as_str().eq_ignore_ascii_case("file"))
    {
        directive.error("sample requires a local file URI for the reader spec");
    } else {
        match formatting::file_path(uri) {
            Ok(base) => match base.parent() {
                Some(parent) => directive.path = Some(parent.join(relative)),
                None => directive.error("reader spec has no parent directory"),
            },
            Err(e) => directive.error(&e.message),
        }
    }
    let mut index = 2;
    let mut unknown = false;
    while index < words.len() {
        work.visit(1);
        let word = &words[index];
        if let Some(selector) = word.strip_prefix("instance=") {
            if selector.is_empty() {
                directive.error("instance= requires a nonempty selector");
            } else {
                directive.params.set("instance", selector);
            }
        } else if word == "instance" {
            directive.error("expected instance=SEL");
        } else if word == "param" {
            index += 1;
            match words.get(index).and_then(|s| s.split_once('=')) {
                Some((name, value)) if !name.is_empty() => directive.params.set(name, value),
                _ => directive.error("param requires NAME=VALUE with a nonempty name"),
            }
        } else {
            unknown = true;
        }
        index += 1;
    }
    if unknown {
        directive.warning("unknown words in sample directive");
    }
    if let Some(selector) = directive.params.get("instance") {
        label.push_str(&format!(" instance={}", label_part(selector, 30)));
    }
    directive.label = format!("{} (directive line {line})", summary::short_text(&label));
    if directive
        .issues
        .iter()
        .any(|i| i.severity == DiagnosticSeverity::ERROR)
    {
        directive.path = None;
    }
    directive
}
impl Directive {
    fn error(&mut self, message: &str) {
        self.issues.push(Issue {
            severity: DiagnosticSeverity::ERROR,
            message: bounded(message, 1024),
        });
    }
    fn warning(&mut self, message: &str) {
        self.issues.push(Issue {
            severity: DiagnosticSeverity::WARNING,
            message: message.into(),
        });
    }
}
/// Double quotes delimit components; backslashes are literal, with no shell expansion.
fn words(text: &str, work: &mut ParseWork) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for character in text.chars() {
        work.visit(character.len_utf8());
        if character == '"' {
            quoted = !quoted;
            started = true;
        } else if matches!(character, ' ' | '\t') && !quoted {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(character);
            started = true;
        }
    }
    if quoted {
        return Err("unterminated double quote in sample directive");
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

#[derive(Clone)]
pub(crate) struct Identity {
    pub uri: Uri,
    pub version: i32,
    pub generation: u64,
    pub cancellation: CancellationToken,
}
pub(crate) struct Job {
    pub identity: Identity,
    pub source: String,
    pub directives: Vec<Directive>,
}
pub(crate) struct Batch {
    pub identity: Identity,
    pub outputs: Option<Vec<ReadOutput>>,
}
pub(crate) enum ReadOutput {
    Skipped,
    Success { preview: String, hint: String },
    Failed(Failure),
    FileError(String),
}

pub(crate) fn execute(job: Job) -> Batch {
    let outputs = read_job(&job);
    Batch {
        identity: job.identity,
        outputs,
    }
}
fn read_job(job: &Job) -> Option<Vec<ReadOutput>> {
    let token = &job.identity.cancellation;
    if token.is_cancelled() {
        return None;
    }
    let spec = Spec::parse(job.source.as_bytes(), job.identity.uri.as_str());
    if token.is_cancelled() {
        return None;
    }
    let spec = match spec {
        Ok(spec) => spec,
        Err(error) => return Some(vec![ReadOutput::Failed(error)]),
    };
    let mut outputs = Vec::with_capacity(job.directives.len());
    for directive in &job.directives {
        if token.is_cancelled() {
            return None;
        }
        let Some(path) = &directive.path else {
            outputs.push(ReadOutput::Skipped);
            continue;
        };
        let input = match acquire(path, token) {
            Ok(bytes) => bytes,
            Err(error) => {
                if token.is_cancelled() {
                    return None;
                }
                outputs.push(ReadOutput::FileError(error.to_string()));
                continue;
            }
        };
        if token.is_cancelled() {
            return None;
        }
        let output = match spec.read_with_options(
            &input,
            &path.to_string_lossy(),
            &directive.params,
            &ReadOptions {
                cancellation: Some(token.clone()),
            },
        ) {
            Ok(data) => {
                if token.is_cancelled() {
                    return None;
                }
                let preview =
                    summary::preview(&directive.label, spec.format_name().as_deref(), &data);
                let hint = summary::short_text(&format!(
                    "{}: {} entries",
                    directive.label,
                    data.entries().len()
                ))
                .replace(['\r', '\n', '\t'], " ");
                ReadOutput::Success { preview, hint }
            }
            Err(Failure::Host(HostError::Cancelled)) => return None,
            Err(error) => ReadOutput::Failed(error),
        };
        if token.is_cancelled() {
            return None;
        }
        outputs.push(output);
    }
    Some(outputs)
}
fn acquire(path: &Path, token: &CancellationToken) -> std::io::Result<Vec<u8>> {
    // Refuse devices/FIFOs before opening. OS metadata/open/read can still block.
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::other("sample is not a regular file"));
    }
    let mut file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("sample is not a regular file"));
    }
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        if token.is_cancelled() {
            return Err(std::io::Error::other("sample read cancelled"));
        }
        let count = file.read(&mut chunk)?;
        if count == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

pub(crate) fn install(document: &mut Document, outputs: Vec<ReadOutput>, positions: &Positions) {
    document.samples.outcomes = outputs
        .into_iter()
        .zip(&document.samples.directives)
        .map(|(output, directive)| match output {
            ReadOutput::Skipped => Outcome::Pending,
            ReadOutput::Success { preview, hint } => Outcome::Success { preview, hint },
            ReadOutput::Failed(error) => Outcome::Failed(crate::diagnostic::sample_failure(
                &error,
                directive,
                &document.source,
                &document.lines,
                positions,
            )),
            ReadOutput::FileError(message) => Outcome::Failed(crate::diagnostic::sample_file(
                &message, directive, positions,
            )),
        })
        .collect();
}

pub(crate) fn bounded(text: &str, bytes: usize) -> String {
    // Inspect at most bytes+one scalar; never count the unbounded suffix.
    let mut out = String::new();
    for c in text.chars() {
        if out.len() + c.len_utf8() > bytes.saturating_sub(3) {
            out.push('…');
            break;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{LineIndex, PositionEncoding};
    #[test]
    fn directive_traversal_and_tokenizing_scale_linearly() {
        let measure = |n: usize| {
            let source = format!(
                "let label=\"{}\" x:int\n{}",
                "a".repeat(n * 32),
                "# iris: sample \"with spaces\" instance=2 param p=3 unknown\n".repeat(n)
            );
            let parsed = gecode_iris_syntax::parse(
                source.as_bytes(),
                "spec",
                gecode_iris_syntax::LanguageVersion::V0_1,
            )
            .unwrap();
            let positions =
                Positions::new(&source, &LineIndex::new(&source), PositionEncoding::Utf16);
            let uri = "file:///tmp/reader.iris".parse().unwrap();
            let mut work = ParseWork::default();
            let directives = parse_counted(parsed.syntax(), &uri, &positions, &mut work);
            assert_eq!(directives.len(), n);
            (source.len(), work.visits)
        };
        let (small_bytes, small) = measure(128);
        let (large_bytes, large) = measure(512);
        assert!(large_bytes >= small_bytes * 3);
        assert!(large <= small * 6, "{small} -> {large}");
    }
    #[test]
    fn quoted_components_preserve_backslashes_and_empty_parameter_values() {
        let mut work = ParseWork::default();
        assert_eq!(
            words(
                r#"sample "a\b c" instance="named item" param p="""#,
                &mut work
            )
            .unwrap(),
            ["sample", "a\\b c", "instance=named item", "param", "p="]
        );
        let markup = markup("``` injected ```".into(), true);
        assert!(markup.value.starts_with("````\n"));
    }
}
