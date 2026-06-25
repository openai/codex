mod array;
mod object;
mod value;

use crate::ApiSchema;
use crate::Arguments;
use crate::Location;
use crate::Method;
use crate::SchemaId;
use crate::SchemaNode;
use crate::SchemaPath;
use crate::SchemaRules;
use crate::SchemaSnapshot;
use crate::Violation;
use anyhow::Result;
use std::collections::BTreeMap;
use std::collections::HashSet;

pub(crate) fn compare_api_schemas(base: &ApiSchema, current: &ApiSchema) -> Result<Vec<Violation>> {
    let mut retained_methods = 0;
    let mut violations = Vec::new();
    for (name, base_method) in &base.methods {
        let Some(current_method) = current.methods.get(name) else {
            violations.push(Violation::MethodRemoved {
                method: name.clone(),
            });
            continue;
        };
        retained_methods += 1;
        let mut cx = CompareCx::new(base, current, name);
        base_method.compare(current_method, &mut cx, &SchemaPath::default())?;
        violations.extend(cx.violations);
    }
    sort_and_dedup(&mut violations);
    Ok(collapse_shared_envelope_changes(
        violations,
        retained_methods,
    ))
}

/// Recursively compares typed schema nodes and records values accepted by the base only.
///
/// Implementations should delegate to child schema types and add narrowly scoped violations to
/// the comparison context rather than interpreting unrelated schema keywords.
pub(super) trait CompareNarrowing<Rhs = Self> {
    fn compare(&self, current: &Rhs, cx: &mut CompareCx<'_>, path: &SchemaPath) -> Result<()>;
}

pub(super) struct CompareCx<'a> {
    pub(super) base: &'a ApiSchema,
    pub(super) current: &'a ApiSchema,
    pub(super) method: &'a str,
    active: HashSet<(SchemaId, SchemaId)>,
    pub(super) violations: Vec<Violation>,
}

impl<'a> CompareCx<'a> {
    fn new(base: &'a ApiSchema, current: &'a ApiSchema, method: &'a str) -> Self {
        Self {
            base,
            current,
            method,
            active: HashSet::new(),
            violations: Vec::new(),
        }
    }

    pub(super) fn location(&self, path: &SchemaPath) -> Location {
        Location::method(self.method, path.clone())
    }

    pub(super) fn constraint_changed(
        &mut self,
        path: &SchemaPath,
        before: serde_json::Value,
        after: serde_json::Value,
    ) {
        self.violations.push(Violation::ConstraintChanged {
            at: self.location(path),
            before: SchemaSnapshot(before),
            after: SchemaSnapshot(after),
        });
    }
}

impl CompareNarrowing for Method {
    fn compare(&self, current: &Self, cx: &mut CompareCx<'_>, path: &SchemaPath) -> Result<()> {
        self.request.compare(&current.request, cx, path)?;
        self.arguments
            .compare(&current.arguments, cx, &path.property("params"))
    }
}

impl CompareNarrowing for Arguments {
    fn compare(&self, current: &Self, cx: &mut CompareCx<'_>, path: &SchemaPath) -> Result<()> {
        match (self.argument(), current.argument()) {
            (None, None) => {}
            (Some(_), None) => cx.violations.push(Violation::PropertyRemoved {
                at: cx.location(path),
            }),
            (None, Some(current)) if current.required => {
                cx.violations.push(Violation::RequiredPropertyAdded {
                    at: cx.location(path),
                });
            }
            (None, Some(_)) => {}
            (Some(base), Some(current)) => {
                if !base.required && current.required {
                    cx.violations.push(Violation::RequiredPropertyAdded {
                        at: cx.location(path),
                    });
                }
                base.schema.compare(&current.schema, cx, path)?;
            }
        }
        Ok(())
    }
}

impl CompareNarrowing for SchemaId {
    fn compare(&self, current: &Self, cx: &mut CompareCx<'_>, path: &SchemaPath) -> Result<()> {
        let (base_id, base) = cx.base.resolve(*self)?;
        let (current_id, current) = cx.current.resolve(*current)?;
        if !cx.active.insert((base_id, current_id)) {
            return Ok(());
        }
        let result = match (base, current) {
            (SchemaNode::Never, _) | (_, SchemaNode::Any) => Ok(()),
            (SchemaNode::Any, _) | (_, SchemaNode::Never) => {
                cx.constraint_changed(
                    path,
                    cx.base.snapshot(base_id)?,
                    cx.current.snapshot(current_id)?,
                );
                Ok(())
            }
            (SchemaNode::Rules(base), SchemaNode::Rules(current)) => {
                compare_rules(base, current, cx, path)
            }
            (SchemaNode::Reference(_), _) | (_, SchemaNode::Reference(_)) => {
                unreachable!("resolve returns concrete schema nodes")
            }
        };
        cx.active.remove(&(base_id, current_id));
        result
    }
}

fn compare_rules(
    base: &SchemaRules,
    current: &SchemaRules,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    value::compare_values(base.values.as_ref(), current.values.as_ref(), cx, path);
    value::compare_types(base.types.as_ref(), current.types.as_ref(), cx, path);
    object::compare_optional(base.object.as_ref(), current.object.as_ref(), cx, path)?;
    array::compare_optional(base.array.as_ref(), current.array.as_ref(), cx, path)?;
    value::compare_constraints(&base.constraints, &current.constraints, cx, path);
    Ok(())
}

fn collapse_shared_envelope_changes(
    violations: Vec<Violation>,
    retained_methods: usize,
) -> Vec<Violation> {
    let mut groups = BTreeMap::<String, Vec<Violation>>::new();
    for violation in violations {
        let mut breakage = violation.breakage();
        breakage.method.clear();
        groups
            .entry(serde_json::to_string(&breakage).unwrap_or_default())
            .or_default()
            .push(violation);
    }

    let mut collapsed = Vec::new();
    for mut group in groups.into_values() {
        let shared = retained_methods > 0
            && group.len() == retained_methods
            && group
                .first()
                .and_then(Violation::location)
                .is_some_and(|location| !location.path.starts_with_params());
        if shared {
            let mut violation = group.remove(0);
            violation.set_shared_envelope();
            collapsed.push(violation);
        } else {
            collapsed.extend(group);
        }
    }
    sort_and_dedup(&mut collapsed);
    collapsed
}

fn sort_and_dedup(violations: &mut Vec<Violation>) {
    violations
        .sort_by_key(|violation| serde_json::to_string(&violation.breakage()).unwrap_or_default());
    violations.dedup();
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
