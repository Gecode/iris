use crate::document::{Document, LineIndex, PositionEncoding};
use gecode_iris_sema::{DefinitionId, DefinitionKind, SemanticModel};
use gecode_iris_syntax::{LanguageVersion, TextRange, TextSize};
use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{
    GotoDefinitionResponse, Location, PrepareRenameResponse, Range, TextDocumentPositionParams,
    TextEdit, Uri, WorkspaceEdit,
    request::{GotoDefinition, PrepareRenameRequest, References, Rename, Request as LspRequest},
};
use std::collections::{BTreeSet, HashMap};

type Result<T> = std::result::Result<T, ResponseError>;

// Uri caches parsed components internally; its identity and hash remain fixed.
#[allow(clippy::mutable_key_type)]
pub(crate) fn handle_request(
    request: Request,
    documents: &mut HashMap<Uri, Document>,
    encoding: PositionEncoding,
) -> Response {
    match request.method.as_str() {
        GotoDefinition::METHOD => respond::<GotoDefinition>(request, |params| {
            let navigation =
                Navigation::new(documents, &params.text_document_position_params, encoding)?;
            let ranges = navigation
                .selected_definitions()
                .iter()
                .map(|id| navigation.model.definition(*id).name_range)
                .collect();
            Ok(Some(GotoDefinitionResponse::Array(
                navigation.locations(ranges)?,
            )))
        }),
        References::METHOD => respond::<References>(request, |params| {
            let navigation = Navigation::new(documents, &params.text_document_position, encoding)?;
            let definitions = navigation.selected_definitions();
            let mut ranges = Vec::new();
            for id in definitions {
                let definition = navigation.model.definition(id);
                if params.context.include_declaration {
                    ranges.push(definition.name_range);
                }
                ranges.extend(
                    definition
                        .references
                        .iter()
                        .map(|id| navigation.model.reference(*id).name_range),
                );
            }
            Ok(Some(navigation.locations(ranges)?))
        }),
        PrepareRenameRequest::METHOD => respond::<PrepareRenameRequest>(request, |params| {
            let navigation = Navigation::new(documents, &params, encoding)?;
            if navigation.selected_definitions().is_empty() {
                return Ok(None);
            }
            // Invalid sources have partial resolution, so cannot establish a rename group.
            navigation.require_valid()?;
            navigation
                .selected_range()
                .map(|range| navigation.range(range).map(PrepareRenameResponse::Range))
                .transpose()
        }),
        Rename::METHOD => respond::<Rename>(request, |params| {
            let navigation = Navigation::new(documents, &params.text_document_position, encoding)?;
            navigation.rename(&params.new_name).map(Some)
        }),
        _ => unreachable!("the server dispatches only navigation requests here"),
    }
}

fn respond<R: LspRequest>(
    request: Request,
    handler: impl FnOnce(R::Params) -> Result<R::Result>,
) -> Response {
    let result = serde_json::from_value(request.params)
        .map_err(|error| invalid_params(error.to_string()))
        .and_then(handler)
        .and_then(|result| serde_json::to_value(result).map_err(|error| failed(error.to_string())));
    Response {
        id: request.id,
        response_result: result,
    }
}

struct Navigation<'a> {
    model: &'a SemanticModel,
    is_valid: bool,
    source: &'a str,
    lines: &'a LineIndex,
    uri: &'a Uri,
    offset: TextSize,
    encoding: PositionEncoding,
}

impl<'a> Navigation<'a> {
    #[allow(clippy::mutable_key_type)] // Uri's internal cache does not change its identity.
    fn new(
        documents: &'a mut HashMap<Uri, Document>,
        params: &'a TextDocumentPositionParams,
        encoding: PositionEncoding,
    ) -> Result<Self> {
        let uri = &params.text_document.uri;
        let document = documents
            .get_mut(uri)
            .ok_or_else(|| invalid_params("document is not open"))?;
        let offset = document
            .lines
            .byte_offset(&document.source, params.position, encoding)
            .map_err(invalid_params)?;
        document
            .analyze(uri.as_str())
            .map_err(|error| failed(error.to_string()))?;
        let analysis = document
            .cached_analysis()
            .expect("analysis was just cached");
        Ok(Self {
            model: &analysis.model,
            is_valid: analysis.is_valid(),
            source: &document.source,
            lines: &document.lines,
            uri,
            offset: TextSize::try_from(offset).map_err(|error| failed(error.to_string()))?,
            encoding,
        })
    }

    fn selected_definitions(&self) -> Vec<DefinitionId> {
        if let Some(id) = self.model.definition_at(self.offset) {
            vec![id]
        } else if let Some(id) = self.model.reference_at(self.offset) {
            self.model.reference(id).resolution.definitions().to_vec()
        } else {
            vec![]
        }
    }

    fn selected_range(&self) -> Option<TextRange> {
        self.model
            .definition_at(self.offset)
            .map(|id| self.model.definition(id).name_range)
            .or_else(|| {
                self.model
                    .reference_at(self.offset)
                    .map(|id| self.model.reference(id).name_range)
            })
    }

    fn require_valid(&self) -> Result<()> {
        if self.is_valid {
            Ok(())
        } else {
            Err(failed(
                "rename requires a reader spec without parse or resolution errors",
            ))
        }
    }

    fn rename(&self, new_name: &str) -> Result<WorkspaceEdit> {
        self.require_valid()?;
        if !is_identifier(new_name) {
            return Err(invalid_params("new name must match [A-Za-z_][A-Za-z0-9_]*"));
        }
        let group = rename_group(self.model, self.selected_definitions());
        if group.is_empty() {
            return Err(failed("position does not select a resolved name"));
        }
        if group
            .iter()
            .all(|id| self.model.definition(*id).name == new_name)
        {
            return Ok(WorkspaceEdit {
                changes: Some(HashMap::from([(self.uri.clone(), vec![])])),
                ..WorkspaceEdit::default()
            });
        }
        if has_value_clash(self.model, &group, new_name) {
            return Err(failed(format!(
                "'{new_name}' clashes with a value outside the rename group"
            )));
        }
        let mut ranges = Vec::new();
        for id in group {
            let definition = self.model.definition(id);
            ranges.push(definition.name_range);
            ranges.extend(
                definition
                    .references
                    .iter()
                    .map(|id| self.model.reference(*id).name_range),
            );
        }
        sort_ranges(&mut ranges);
        validate_rename(
            self.model,
            self.source,
            &ranges,
            new_name,
            self.uri.as_str(),
        )?;
        let edits = ranges
            .into_iter()
            .map(|range| {
                Ok(TextEdit {
                    range: self.range(range)?,
                    new_text: new_name.into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(WorkspaceEdit {
            changes: Some(HashMap::from([(self.uri.clone(), edits)])),
            ..WorkspaceEdit::default()
        })
    }

    fn range(&self, range: TextRange) -> Result<Range> {
        Ok(Range::new(
            self.lines
                .position(self.source, usize::from(range.start()), self.encoding)
                .map_err(failed)?,
            self.lines
                .position(self.source, usize::from(range.end()), self.encoding)
                .map_err(failed)?,
        ))
    }

    fn locations(&self, mut ranges: Vec<TextRange>) -> Result<Vec<Location>> {
        sort_ranges(&mut ranges);
        ranges
            .into_iter()
            .map(|range| Ok(Location::new(self.uri.clone(), self.range(range)?)))
            .collect()
    }
}

fn rename_group(model: &SemanticModel, mut pending: Vec<DefinitionId>) -> BTreeSet<DefinitionId> {
    let mut group = BTreeSet::new();
    // A reference shared by many definitions is expanded only once.
    let mut expanded = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if group.insert(id) {
            for reference in &model.definition(id).references {
                if expanded.insert(*reference) {
                    pending.extend(model.reference(*reference).resolution.definitions());
                }
            }
        }
    }
    group
}

fn is_value(kind: DefinitionKind) -> bool {
    matches!(
        kind,
        DefinitionKind::Read
            | DefinitionKind::Capture
            | DefinitionKind::Let
            | DefinitionKind::Param
            | DefinitionKind::Store
    )
}

fn is_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn has_value_clash(model: &SemanticModel, group: &BTreeSet<DefinitionId>, new_name: &str) -> bool {
    // All ordinary definitions ultimately publish into the enclosing frames,
    // including repeat columns. A different definition of the new spelling
    // would merge entries even when no reference currently observes them.
    group.iter().any(|id| is_value(model.definition(*id).kind))
        && model
            .definitions()
            .iter()
            .enumerate()
            .any(|(index, other)| {
                other.name == new_name
                    && is_value(other.kind)
                    && !group.contains(&DefinitionId(index))
            })
}

fn sort_ranges(ranges: &mut Vec<TextRange>) {
    ranges.sort_unstable_by_key(|range| (range.start(), range.end()));
    ranges.dedup();
}

/// Let the language layers enforce contextual keywords and structural/type/binding
/// conflicts. Compare their resulting edges as well: a valid rename must not
/// capture an unresolved reference or silently change another name's targets.
fn validate_rename(
    model: &SemanticModel,
    source: &str,
    ranges: &[TextRange],
    new_name: &str,
    origin: &str,
) -> Result<()> {
    let mut renamed = source.to_owned();
    for range in ranges.iter().rev() {
        renamed.replace_range(
            usize::from(range.start())..usize::from(range.end()),
            new_name,
        );
    }
    let parsed = gecode_iris_syntax::parse(renamed.as_bytes(), origin, LanguageVersion::V0_1)
        .map_err(|error| failed(error.to_string()))?;
    let analysis = gecode_iris_sema::analyze(&parsed);
    if let Some(error) = analysis.first_error() {
        return Err(failed(format!(
            "cannot rename to '{new_name}': {}",
            error.message
        )));
    }
    check_bindings(model, &analysis.model, ranges, new_name.len())
}

fn check_bindings(
    model: &SemanticModel,
    renamed_model: &SemanticModel,
    ranges: &[TextRange],
    new_name_len: usize,
) -> Result<()> {
    let shift = RenameShift::new(ranges, new_name_len);
    // IDs belong to one snapshot. Correspondence is established by raw name
    // ranges and definition kinds, never by equating IDs across analyses.
    let expected_definitions: Vec<TextRange> = model
        .definitions()
        .iter()
        .map(|definition| shift.translate(definition.name_range))
        .collect();
    let mut renamed_definitions: HashMap<TextRange, Vec<DefinitionKind>> = HashMap::new();
    for definition in renamed_model.definitions() {
        renamed_definitions
            .entry(definition.name_range)
            .or_default()
            .push(definition.kind);
    }
    if model.definitions().len() != renamed_model.definitions().len()
        || model.references().len() != renamed_model.references().len()
        || model
            .definitions()
            .iter()
            .zip(&expected_definitions)
            .any(|(definition, range)| {
                !renamed_definitions
                    .get(range)
                    .is_some_and(|kinds| kinds.contains(&definition.kind))
            })
    {
        return Err(failed("new name changes the reader spec's name structure"));
    }
    let renamed_references: HashMap<TextRange, _> = renamed_model
        .references()
        .iter()
        .map(|reference| (reference.name_range, reference))
        .collect();
    for reference in model.references() {
        let Some(other) = renamed_references.get(&shift.translate(reference.name_range)) else {
            return Err(failed("new name changes the reader spec's references"));
        };
        let mut expected: Vec<_> = reference
            .resolution
            .definitions()
            .iter()
            .map(|id| expected_definitions[id.0])
            .collect();
        let mut actual: Vec<_> = other
            .resolution
            .definitions()
            .iter()
            .map(|id| renamed_model.definition(*id).name_range)
            .collect();
        sort_ranges(&mut expected);
        sort_ranges(&mut actual);
        if expected != actual {
            return Err(failed(
                "new name would change which definitions a reference can target",
            ));
        }
    }
    Ok(())
}

/// Maps offsets in the original source to the renamed source.
struct RenameShift<'a> {
    /// Sorted, non-overlapping replaced ranges.
    edits: &'a [TextRange],
    /// `growth[i]` is the length change from the first `i` edits.
    growth: Vec<isize>,
}

impl<'a> RenameShift<'a> {
    fn new(edits: &'a [TextRange], new_len: usize) -> Self {
        let mut growth = Vec::with_capacity(edits.len() + 1);
        growth.push(0);
        for edit in edits {
            let last = growth[growth.len() - 1];
            growth.push(last + new_len as isize - usize::from(edit.len()) as isize);
        }
        Self { edits, growth }
    }

    fn translate(&self, range: TextRange) -> TextRange {
        TextRange::new(self.offset(range.start()), self.offset(range.end()))
    }

    fn offset(&self, offset: TextSize) -> TextSize {
        let preceding = self.edits.partition_point(|edit| edit.end() <= offset);
        let translated = usize::from(offset).wrapping_add_signed(self.growth[preceding]);
        TextSize::try_from(translated)
            .expect("the renamed source was parsed within the source size limit")
    }
}

fn invalid_params(message: impl Into<String>) -> ResponseError {
    ResponseError {
        code: ErrorCode::InvalidParams as i32,
        message: message.into(),
        data: None,
    }
}

fn failed(message: impl Into<String>) -> ResponseError {
    ResponseError {
        code: ErrorCode::RequestFailed as i32,
        message: message.into(),
        data: None,
    }
}
