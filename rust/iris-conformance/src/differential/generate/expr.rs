//! Expressions over the names a statement can see.

use super::{Context, Generator, Kind, Records, View, WORDS, parenthesize};

impl Generator {
    /// The names visible from the current statement in `context`, as
    /// Specification §8.12 resolves them. In a check, fields of one
    /// completed repeat lift to their per-record values; the others are
    /// left out, so no check mixes repeats.
    pub(super) fn views(&self, context: Context, keep: impl Fn(&View) -> bool) -> Vec<View> {
        let focus = self.check_focus();
        self.names
            .iter()
            .filter(|name| !name.optional)
            .filter_map(|name| {
                let common = name
                    .repeats
                    .iter()
                    .zip(&self.active)
                    .take_while(|(left, right)| left == right)
                    .count();
                let remaining = &name.repeats[common..];
                let depth = match context {
                    Context::Ordinary => name.own_depth + remaining.len(),
                    Context::Check if remaining.is_empty() => name.own_depth,
                    Context::Check if remaining.len() == 1 && Some(remaining[0]) == focus => {
                        name.own_depth
                    }
                    Context::Check => return None,
                };
                Some(View {
                    ident: name.ident.clone(),
                    kind: name.kind,
                    depth,
                })
            })
            .filter(|view| keep(view))
            .collect()
    }

    /// The completed repeat whose fields a check may lift: the latest one.
    fn check_focus(&self) -> Option<usize> {
        self.names.iter().rev().find_map(|name| {
            let common = name
                .repeats
                .iter()
                .zip(&self.active)
                .take_while(|(left, right)| left == right)
                .count();
            (name.repeats.len() == common + 1).then(|| name.repeats[common])
        })
    }

    pub(super) fn pick_view(&mut self, views: &[View]) -> Option<View> {
        (!views.is_empty()).then(|| self.rng.pick(views).clone())
    }

    pub(super) fn int_expr(&mut self, context: Context, budget: u32) -> String {
        let scalars = self.views(context, |view| view.depth == 0 && view.kind == Kind::Int);
        let arrays = self.views(context, |view| view.depth == 1 && view.kind == Kind::Int);
        let any_arrays = self.views(context, |view| view.depth >= 1);
        let choice = if budget == 0 {
            self.rng.below(2)
        } else {
            self.rng.below(13)
        };
        match choice {
            0 => self.rng.between(-2, 9).to_string(),
            1 => match self.pick_view(&scalars) {
                Some(view) => view.ident,
                None => self.rng.between(0, 5).to_string(),
            },
            2 | 3 => {
                let op = *self.rng.pick(&["+", "-", "*"]);
                format!(
                    "{} {op} {}",
                    self.int_expr(context, budget - 1),
                    self.int_operand(context, budget - 1)
                )
            }
            4 => {
                let op = *self.rng.pick(&["/", "%"]);
                let divisor = *self.rng.pick(&[1, 2, 3, -2, 7]);
                format!("{} {op} {divisor}", self.int_operand(context, budget - 1))
            }
            5 => format!("-{}", self.int_operand(context, budget - 1)),
            6 => {
                let function = *self.rng.pick(&["min", "max", "sum"]);
                format!(
                    "{function}({}, {})",
                    self.int_expr(context, budget - 1),
                    self.int_expr(context, budget - 1)
                )
            }
            7 => match self.pick_view(&arrays) {
                Some(view) => {
                    let function = *self.rng.pick(&["sum", "count", "size", "min", "max"]);
                    format!("{function}({})", view.ident)
                }
                None => self.rng.between(0, 3).to_string(),
            },
            8 => match self.pick_view(&arrays) {
                Some(view) => {
                    let function = *self.rng.pick(&["sum", "count"]);
                    let body = if function == "sum" { "v * 2" } else { "v > 0" };
                    format!("{function}(v in {}: {body})", view.ident)
                }
                None => "0".into(),
            },
            9 => match self.pick_view(&arrays) {
                Some(view) => format!("{}[{}]", view.ident, self.rng.between(0, 1)),
                None => "1".into(),
            },
            10 => self.records_expr(context),
            11 => match self.pick_view(&any_arrays) {
                Some(view) => format!("size({})", view.ident),
                None => "gecode.int_max".into(),
            },
            _ => format!("{} ^ 2", self.int_operand(context, budget - 1)),
        }
    }

    /// An int expression that binds tighter than any binary operator.
    fn int_operand(&mut self, context: Context, budget: u32) -> String {
        let expression = self.int_expr(context, budget);
        parenthesize(expression)
    }

    /// An int expression over the records of a completed `repeat ... as`.
    fn records_expr(&mut self, context: Context) -> String {
        let usable: Vec<Records> = self
            .records
            .iter()
            .filter(|records| records.repeats == self.active)
            .cloned()
            .collect();
        if usable.is_empty() {
            return "3".into();
        }
        let records = self.rng.pick(&usable).clone();
        let ints: Vec<&(String, Kind)> = records
            .fields
            .iter()
            .filter(|(_, kind)| *kind == Kind::Int)
            .collect();
        let field = (!ints.is_empty()).then(|| ints[self.rng.below(ints.len())].0.clone());
        match (self.rng.below(4), field, &records.each) {
            (0, Some(field), _) => format!("sum(r in {}: r.{field})", records.ident),
            (1, Some(field), _) => format!("{}[0].{field}", records.ident),
            (2, Some(field), Some(each))
                if context == Context::Check && self.check_focus() == Some(records.repeat) =>
            {
                format!("{each}.{field}")
            }
            _ => format!("size({})", records.ident),
        }
    }

    /// A numeric scalar expression. Iris 0.1 has no float arithmetic
    /// (Specification §8.5), so floats appear as literals, names, negations
    /// and arguments of min and max, and rarely as an operand of an
    /// integer operator, which fails with E-INTEGER-TYPE.
    pub(super) fn numeric_expr(&mut self, context: Context, budget: u32) -> String {
        let scalars = self.views(context, |view| view.depth == 0 && view.kind.is_numeric());
        match self.rng.below(if budget == 0 { 3 } else { 7 }) {
            0 => (*self.rng.pick(&["1.5", "-0.0", "2e3", "0.1", "1e-300"])).to_string(),
            1 => match self.pick_view(&scalars) {
                Some(view) => view.ident,
                None => "2.5".into(),
            },
            2 => self.int_operand(context, 0),
            3 => format!("-{}", parenthesize(self.numeric_expr(context, budget - 1))),
            4 | 5 => {
                let function = *self.rng.pick(&["min", "max"]);
                format!(
                    "{function}({}, {})",
                    self.numeric_expr(context, budget - 1),
                    self.numeric_expr(context, budget - 1)
                )
            }
            _ if self.rng.chance(20) => format!(
                "{} + 1",
                parenthesize(self.numeric_expr(context, budget - 1))
            ),
            _ => self.int_expr(context, budget - 1),
        }
    }

    pub(super) fn string_expr(&mut self, context: Context) -> String {
        let scalars = self.views(context, |view| view.depth == 0 && view.kind == Kind::Str);
        match self.pick_view(&scalars) {
            Some(view) if self.rng.chance(70) => view.ident,
            _ => format!("\"{}\"", self.rng.pick(&WORDS)),
        }
    }

    pub(super) fn bool_expr(&mut self, context: Context, budget: u32) -> String {
        let arrays = self.views(context, |view| view.depth == 1 && view.kind.is_numeric());
        let choice = if budget == 0 {
            self.rng.below(3)
        } else {
            self.rng.below(10)
        };
        match choice {
            0 => {
                let op = *self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
                format!(
                    "{} {op} {}",
                    self.int_expr(context, budget.min(1)),
                    self.int_expr(context, budget.min(1))
                )
            }
            1 => {
                let op = *self.rng.pick(&["<", ">=", "!="]);
                format!(
                    "{} {op} {}",
                    parenthesize(self.numeric_expr(context, budget.min(1))),
                    parenthesize(self.numeric_expr(context, budget.min(1)))
                )
            }
            2 => {
                let op = *self.rng.pick(&["==", "!="]);
                format!(
                    "{} {op} {}",
                    self.string_expr(context),
                    self.string_expr(context)
                )
            }
            3 | 4 => {
                let op = *self.rng.pick(&["&&", "||"]);
                format!(
                    "({}) {op} ({})",
                    self.bool_expr(context, budget - 1),
                    self.bool_expr(context, budget - 1)
                )
            }
            5 => format!("!({})", self.bool_expr(context, budget - 1)),
            6 => match self.pick_view(&arrays) {
                Some(view) => {
                    let function = *self.rng.pick(&["all", "any"]);
                    format!("{function}(v in {}: v >= 0)", view.ident)
                }
                None => "1".into(),
            },
            7 => match self.pick_view(&arrays) {
                Some(view) => format!("distinct({})", view.ident),
                None => "distinct(1, 2)".into(),
            },
            8 if !self.names.is_empty() => {
                let name = self.names[self.rng.below(self.names.len())].ident.clone();
                format!("defined({name})")
            }
            _ => self.int_expr(context, budget - 1),
        }
    }
}
