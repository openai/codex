use super::CompareCx;
use super::CompareNarrowing;
use crate::AdditionalItems;
use crate::ArraySchema;
use crate::Items;
use crate::SchemaId;
use crate::SchemaPath;
use anyhow::Result;
use serde_json::Value;

pub(super) fn compare_optional(
    base: Option<&ArraySchema>,
    current: Option<&ArraySchema>,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    if base.is_none() && current.is_none() {
        return Ok(());
    }
    let unconstrained = ArraySchema {
        items: Items::Any,
        additional_items: AdditionalItems::Any,
    };
    compare(
        base.unwrap_or(&unconstrained),
        current.unwrap_or(&unconstrained),
        cx,
        path,
    )
}

fn compare(
    base: &ArraySchema,
    current: &ArraySchema,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    let tuple_len = tuple_len(&base.items).max(tuple_len(&current.items));
    for index in 0..tuple_len {
        compare_element(
            element_at(base, index),
            element_at(current, index),
            cx,
            &path.tuple_item(index),
        )?;
    }
    compare_element(tail(base), tail(current), cx, &path.items())
}

#[derive(Clone, Copy)]
enum Element {
    Any,
    Forbidden,
    Schema(SchemaId),
}

fn tuple_len(items: &Items) -> usize {
    match items {
        Items::Tuple(items) => items.len(),
        Items::Any | Items::Each(_) => 0,
    }
}

fn element_at(array: &ArraySchema, index: usize) -> Element {
    match &array.items {
        Items::Any => Element::Any,
        Items::Each(schema) => Element::Schema(*schema),
        Items::Tuple(items) => items
            .get(index)
            .copied()
            .map_or_else(|| additional(array.additional_items), Element::Schema),
    }
}

fn tail(array: &ArraySchema) -> Element {
    match array.items {
        Items::Any => Element::Any,
        Items::Each(schema) => Element::Schema(schema),
        Items::Tuple(_) => additional(array.additional_items),
    }
}

fn additional(items: AdditionalItems) -> Element {
    match items {
        AdditionalItems::Any => Element::Any,
        AdditionalItems::Forbidden => Element::Forbidden,
        AdditionalItems::Schema(schema) => Element::Schema(schema),
    }
}

fn compare_element(
    base: Element,
    current: Element,
    cx: &mut CompareCx<'_>,
    path: &SchemaPath,
) -> Result<()> {
    match (base, current) {
        (Element::Forbidden, _) | (_, Element::Any) => Ok(()),
        (Element::Schema(base), Element::Schema(current)) => base.compare(&current, cx, path),
        (Element::Any, Element::Forbidden) => {
            cx.constraint_changed(path, Value::Bool(true), Value::Bool(false));
            Ok(())
        }
        (Element::Any, Element::Schema(current)) => {
            cx.constraint_changed(path, Value::Bool(true), cx.current.snapshot(current)?);
            Ok(())
        }
        (Element::Schema(base), Element::Forbidden) => {
            cx.constraint_changed(path, cx.base.snapshot(base)?, Value::Bool(false));
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "array_tests.rs"]
mod tests;
