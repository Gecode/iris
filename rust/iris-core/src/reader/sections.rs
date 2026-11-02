use super::*;

#[derive(Debug)]
pub(super) struct Sections {
    node: SyntaxNode,
    alternatives: Vec<String>,
    store: Option<Name>,
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
    pub fn new(
        node: SyntaxNode,
        strings: &HashMap<TextRange, String>,
        names: &HashMap<TextRange, Name>,
    ) -> Self {
        let mut alternatives = vec![];
        let mut store = None;
        for option in node.children().filter(|n| n.kind() == K::Terminators) {
            let tokens = tokens(&option);
            alternatives.extend(
                tokens
                    .iter()
                    .filter(|t| t.kind() == K::String)
                    .map(|t| strings[&t.text_range()].clone()),
            );
            if let Some(index) = tokens.iter().position(|t| t.text() == "store") {
                store = Some(names[&tokens[index + 1].text_range()].clone());
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
        self.input.skip_to_data()?;
        if self.input.peek_byte().is_none() {
            return self.finish_sections(self.input.position(), "the end of the input");
        }
        if let Some(position) = self.take_sections_terminator(sections)? {
            let end = self
                .input
                .control
                .quoted_alternatives(&sections.alternatives)?;
            return self.finish_sections(position, &end);
        }
        let line = self.input.remaining_line()?.expect("data remains");
        for (index, arm) in sections.arms.iter().enumerate() {
            let pattern = &self.spec.index.patterns[&arm.pattern];
            if let Some(captures) = self.match_pattern(pattern, &line)? {
                if let Some(first) = self.sections.last().expect("active sections").seen[index] {
                    let error = self.language_error_at(
                        ErrorCode::SectionsRepeated,
                        self.input.control.text_message(&[
                            "a second line matching \"",
                            &pattern.source,
                            &format!("\" (the first is on line {})", first.line),
                        ])?,
                        line.start,
                    )?;
                    self.reject(
                        error,
                        self.input.control.text_message(&[
                            "a second line matching \"",
                            &pattern.source,
                            "\"",
                        ])?,
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
                    work.push(Work::Statement(self.spec.index.plan.id(body)));
                }
                return Ok(());
            }
        }
        if sections.skip_other {
            self.input.consume_line(&line)?;
            work.push(Work::SectionsNext);
            return Ok(());
        }
        let mut message = String::from("unexpected line; expected one of ");
        for (index, arm) in sections.arms.iter().take(6).enumerate() {
            if index > 0 {
                message.push_str(", ");
            }
            message.push('"');
            self.input.control.push_text(
                &mut message,
                self.spec.index.patterns[&arm.pattern].source.as_bytes(),
            )?;
            message.push('"');
        }
        if sections.arms.len() > 6 {
            message.push_str(", ...");
        }
        Err(self.error_at(ErrorCode::SectionsUnexpected, message, line.start))
    }
    fn take_sections_terminator(
        &mut self,
        sections: &Sections,
    ) -> Result<Option<Position>, Failure> {
        if sections.alternatives.is_empty() {
            return Ok(None);
        }
        let Some(token) = self.input.peek_token()? else {
            return Ok(None);
        };
        if !self.matches_token(&sections.alternatives, token.bytes)? {
            return Ok(None);
        }
        self.input.next_token()?;
        if let Some(name) = &sections.store {
            self.frames
                .last_mut()
                .expect("active frame")
                .set_controlled(
                    name.clone(),
                    Value::string(
                        self.input
                            .control
                            .copy_utf8(token.bytes)?
                            .expect("literal terminator matched"),
                    ),
                    &self.input.control,
                )?;
        }
        Ok(Some(token.start))
    }
    pub(super) fn after_section_body(&mut self, work: &mut Vec<Work>) -> Result<(), Failure> {
        self.contexts.pop();
        let state = self.sections.last().expect("active sections");
        let index = state.selected.expect("selected arm");
        let arm = &self.spec.index.sections[&state.descriptor].arms[index];
        if arm.last {
            let position = state.seen[index].expect("matched arm");
            let end = self.input.control.text_message(&[
                "the line matching \"",
                &self.spec.index.patterns[&arm.pattern].source,
                "\"",
            ])?;
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
                let error = self.language_error_at(
                    ErrorCode::LineMatch,
                    self.input.control.text_message(&[
                        "expected a line matching \"",
                        &self.spec.index.patterns[&arm.pattern].source,
                        "\" before ",
                        end,
                    ])?,
                    position,
                )?;
                let summary = self.input.control.text_message(&[
                    "no line matching \"",
                    &self.spec.index.patterns[&arm.pattern].source,
                    "\"",
                ])?;
                self.input.skip_to_data()?;
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
