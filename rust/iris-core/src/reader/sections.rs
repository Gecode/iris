use super::*;

#[derive(Debug)]
pub(super) struct Sections {
    node: SyntaxNode,
    alternatives: Vec<String>,
    store: Option<String>,
    skip_other: bool,
    arms: Vec<Arm>,
}
#[derive(Debug)]
struct Arm {
    pattern: TextRange,
    optional: bool,
    last: bool,
    body: Option<SyntaxNode>,
}
impl Sections {
    pub fn new(node: SyntaxNode) -> Self {
        let mut alternatives = vec![];
        let mut store = None;
        for option in node.children().filter(|n| n.kind() == K::Terminators) {
            let tokens = tokens(&option);
            alternatives.extend(
                tokens
                    .iter()
                    .filter(|t| t.kind() == K::String)
                    .map(|t| DecodedString::new(t.text()).text),
            );
            if let Some(index) = tokens.iter().position(|t| t.text() == "store") {
                store = Some(tokens[index + 1].text().to_owned());
            }
        }
        let skip_other = tokens(&node).iter().any(|t| t.text() == "skip");
        let arms = node
            .children()
            .filter(|n| n.kind() == K::Arm)
            .map(|arm| {
                let tokens = tokens(&arm);
                Arm {
                    pattern: child(&arm, K::Pattern).text_range(),
                    optional: tokens.iter().any(|t| t.text() == "optional"),
                    last: tokens.iter().any(|t| t.text() == "last"),
                    body: arm.children().find(|n| n.kind() == K::Block),
                }
            })
            .collect();
        Self {
            node,
            alternatives,
            store,
            skip_other,
            arms,
        }
    }
}
pub(super) struct SectionsState {
    descriptor: TextRange,
    seen: Vec<Option<Position>>,
    selected: Option<usize>,
}
impl Interpreter<'_, '_> {
    pub(super) fn begin_sections(&mut self, node: &SyntaxNode) {
        let descriptor = node.text_range();
        self.sections.push(SectionsState {
            descriptor,
            seen: vec![None; self.spec.index.sections[&descriptor].arms.len()],
            selected: None,
        });
    }
    pub(super) fn next_section(&mut self, work: &mut Vec<Work>) -> Result<(), Failure> {
        let state = self.sections.last().expect("active sections");
        let sections = &self.spec.index.sections[&state.descriptor];
        self.select_statement(&sections.node);
        self.input.skip_to_data();
        if self.input.peek_byte().is_none() {
            return self.finish_sections(self.input.position(), "the end of the input");
        }
        if let Some(position) = self.take_sections_terminator(sections) {
            let end = sections
                .alternatives
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(" or ");
            return self.finish_sections(position, &end);
        }
        let line = self.input.remaining_line().expect("data remains");
        for (index, arm) in sections.arms.iter().enumerate() {
            let pattern = &self.spec.index.patterns[&arm.pattern];
            if let Some(captures) = self.match_pattern(pattern, &line)? {
                if let Some(first) = self.sections.last().expect("active sections").seen[index] {
                    let error = self.error_at(
                        ErrorCode::SectionsRepeated,
                        format!(
                            "a second line matching \"{}\" (the first is on line {})",
                            pattern.source, first.line
                        ),
                        line.start,
                    );
                    self.reject(
                        error,
                        format!("a second line matching \"{}\"", pattern.source),
                    )?;
                }
                let state = self.sections.last_mut().expect("active sections");
                if state.seen[index].is_none() {
                    state.seen[index] = Some(line.start);
                }
                state.selected = Some(index);
                self.take_pattern(pattern, &line, captures)?;
                self.contexts.push(ExecutionContext::Section(arm.pattern));
                work.push(Work::AfterSectionBody);
                if let Some(body) = &arm.body {
                    work.push(Work::Statement(body.clone()));
                }
                return Ok(());
            }
        }
        if sections.skip_other {
            self.input.consume_line(&line);
            work.push(Work::SectionsNext);
            return Ok(());
        }
        let mut expected = sections
            .arms
            .iter()
            .take(6)
            .map(|arm| format!("\"{}\"", self.spec.index.patterns[&arm.pattern].source))
            .collect::<Vec<_>>()
            .join(", ");
        if sections.arms.len() > 6 {
            expected.push_str(", ...");
        }
        Err(self
            .error_at(
                ErrorCode::SectionsUnexpected,
                format!("unexpected line; expected one of {expected}"),
                line.start,
            )
            .into())
    }
    fn take_sections_terminator(&mut self, sections: &Sections) -> Option<Position> {
        if sections.alternatives.is_empty() {
            return None;
        }
        let token = self.input.peek_token()?;
        if !sections
            .alternatives
            .iter()
            .any(|s| s.as_bytes() == token.bytes)
        {
            return None;
        }
        self.input.next_token();
        if let Some(name) = &sections.store {
            self.frames.last_mut().expect("active frame").set(
                name.clone(),
                Value::string(
                    std::str::from_utf8(token.bytes)
                        .expect("literal terminator matched")
                        .to_owned(),
                ),
            );
        }
        Some(token.start)
    }
    pub(super) fn after_section_body(&mut self, work: &mut Vec<Work>) -> Result<(), Failure> {
        self.contexts.pop();
        let state = self.sections.last().expect("active sections");
        let index = state.selected.expect("selected arm");
        let arm = &self.spec.index.sections[&state.descriptor].arms[index];
        if arm.last {
            let position = state.seen[index].expect("matched arm");
            let end = format!(
                "the line matching \"{}\"",
                self.spec.index.patterns[&arm.pattern].source
            );
            self.finish_sections(position, &end)
        } else {
            work.push(Work::SectionsNext);
            Ok(())
        }
    }
    fn finish_sections(&mut self, position: Position, end: &str) -> Result<(), Failure> {
        let state = self.sections.last().expect("completing sections");
        let sections = &self.spec.index.sections[&state.descriptor];
        for (arm, seen) in sections.arms.iter().zip(&state.seen) {
            if !arm.optional && seen.is_none() {
                let error = self.error_at(
                    ErrorCode::LineMatch,
                    format!(
                        "expected a line matching \"{}\" before {end}",
                        self.spec.index.patterns[&arm.pattern].source
                    ),
                    position,
                );
                let summary = format!(
                    "no line matching \"{}\"",
                    self.spec.index.patterns[&arm.pattern].source
                );
                self.input.skip_to_data();
                if self.input.peek_byte().is_none() {
                    return Err(error.into());
                }
                self.reject(error, summary)?;
                self.sections.pop();
                return Ok(());
            }
        }
        self.sections.pop();
        Ok(())
    }
}
