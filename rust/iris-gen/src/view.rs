//! The typed handles of a reader spec, as a list that belongs to no language.
//!
//! A [`View`] says what a spec's handles promise, and a backend prints it as
//! declarations. Every fact comes from the output entries of `gecode-iris-sema`.
//! A typed handle is a promise that reading through it cannot fail on data that a
//! read produces, so a name gets [`Access::Dynamic`] whenever the facts leave
//! more than one value type or depth open.
use gecode_iris_sema::{Analysis, Error, OutputEntry, Possibilities, SemanticModel, ValueType};

/// The handles of a reader spec, in the order of the entries' first definitions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    pub handles: Vec<Handle>,
}

/// The handle of one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle {
    /// The entry's name in the data, whatever a language has to call the handle.
    pub entry: String,
    pub access: Access,
}

/// What reading through a handle gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// A value of one value type and depth. `optional` means that the entry
    /// may be absent.
    Typed {
        value: Leaf,
        depth: usize,
        optional: bool,
    },
    /// The language's dynamic value, for an entry whose shape stays open.
    Dynamic,
}

/// The value type of a typed handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaf {
    Int,
    Float,
    Str,
    /// Int or float, decided by the data.
    Num,
}

/// The view of a spec, or the first error that `iris check` reports.
pub fn view(analysis: &Analysis) -> Result<View, Error> {
    if let Some(error) = analysis.first_error() {
        return Err(error.clone());
    }
    let model = &analysis.model;
    let handles = model
        .output_entries()
        .iter()
        .map(|entry| Handle {
            entry: entry.name.clone(),
            access: access(entry, model),
        })
        .collect();
    Ok(View { handles })
}

fn access(entry: &OutputEntry, model: &SemanticModel) -> Access {
    let Possibilities::Known(depths) = &entry.facts.depths else {
        return Access::Dynamic;
    };
    match (depths.as_slice(), leaf(entry, model)) {
        (&[depth], Some(value)) => Access::Typed {
            value,
            depth,
            optional: entry.possibly_absent,
        },
        _ => Access::Dynamic,
    }
}

/// The one value type of `entry`. Int and float together are [`Leaf::Num`] only
/// when each definition leaves both open: a name redefined from int to float has
/// another value type, as `docs/builders.md` says, and so is dynamic.
fn leaf(entry: &OutputEntry, model: &SemanticModel) -> Option<Leaf> {
    let Possibilities::Known(value_types) = &entry.facts.value_types else {
        return None;
    };
    match value_types.as_slice() {
        [ValueType::Int] => Some(Leaf::Int),
        [ValueType::Float] => Some(Leaf::Float),
        [ValueType::String] => Some(Leaf::Str),
        [_, _] if is_numeric(value_types) => entry
            .definitions
            .iter()
            .all(|id| {
                matches!(
                    &model.facts_for_definition(*id).value_types,
                    Possibilities::Known(own) if is_numeric(own)
                )
            })
            .then_some(Leaf::Num),
        _ => None,
    }
}

/// Whether `value_types`, a set, is exactly int and float.
fn is_numeric(value_types: &[ValueType]) -> bool {
    value_types.len() == 2 && !value_types.contains(&ValueType::String)
}
