use anyhow::Context;
use anyhow::Result;
use serde_json::Value;
use serde_json::json;

use crate::actions::BrowserToolOutput;
use crate::actions::bounded_snapshot_json;
use crate::actions::page_metadata;
use crate::cdp::CdpClient;
use crate::handles::BrowserHandles;
use crate::key_event;

const MAX_AX_NODES: usize = 300;
const MAX_SNAPSHOT_TEXT_CHARS: usize = 6_000;

pub(crate) async fn snapshot(
    client: &CdpClient,
    handles: &mut BrowserHandles,
) -> Result<BrowserToolOutput> {
    let tree = client
        .call("Accessibility.getFullAXTree", json!({}))
        .await?;
    let metadata = page_metadata(client).await?;
    let ax_nodes = tree
        .get("nodes")
        .and_then(Value::as_array)
        .context("accessibility tree omitted nodes")?;
    handles.begin_snapshot();
    let mut actionable_nodes = Vec::new();
    let mut contextual_nodes = Vec::new();
    let mut page_text = String::new();
    for (index, node) in ax_nodes.iter().enumerate().filter(|(_, node)| {
        !node
            .get("ignored")
            .and_then(Value::as_bool)
            .unwrap_or(/*default*/ false)
    }) {
        let role = ax_value(node, "role");
        let name = ax_value(node, "name");
        if role == "StaticText" && !name.is_empty() && page_text.len() < MAX_SNAPSHOT_TEXT_CHARS {
            if !page_text.is_empty() {
                page_text.push('\n');
            }
            let remaining = MAX_SNAPSHOT_TEXT_CHARS.saturating_sub(page_text.len());
            page_text.extend(name.chars().take(remaining));
        }
        let actionable = is_actionable_role(&role);
        if (role.is_empty() && name.is_empty())
            || (actionable && actionable_nodes.len() >= MAX_AX_NODES)
        {
            continue;
        }
        let mut output = serde_json::Map::new();
        output.insert("role".to_string(), Value::String(role.clone()));
        output.insert(
            "name".to_string(),
            Value::String(clipped(&name, /*max_chars*/ 200)),
        );
        if actionable
            && let Some(backend_node_id) = node.get("backendDOMNodeId").and_then(Value::as_u64)
        {
            output.insert(
                "nodeId".to_string(),
                Value::String(handles.insert(backend_node_id)),
            );
        }
        if let Some(disabled) = ax_property(node, "disabled").and_then(Value::as_bool) {
            output.insert("disabled".to_string(), Value::Bool(disabled));
        }
        if let Some(checked) = ax_property(node, "checked") {
            output.insert("checked".to_string(), checked.clone());
        }
        let value = ax_value(node, "value");
        if !value.is_empty() {
            output.insert(
                "value".to_string(),
                Value::String(if is_editable_role(&role) {
                    "<redacted>".to_string()
                } else {
                    clipped(&value, /*max_chars*/ 200)
                }),
            );
        }
        if actionable {
            actionable_nodes.push((index, Value::Object(output)));
        } else if contextual_nodes.len() < MAX_AX_NODES {
            contextual_nodes.push((index, Value::Object(output)));
        }
    }
    contextual_nodes.truncate(MAX_AX_NODES.saturating_sub(actionable_nodes.len()));
    actionable_nodes.extend(contextual_nodes);
    actionable_nodes.sort_by_key(|(index, _)| *index);
    let nodes = actionable_nodes
        .into_iter()
        .map(|(_, node)| node)
        .collect::<Vec<_>>();
    let snapshot = json!({
        "url": metadata.url,
        "title": metadata.title,
        "nodes": nodes,
        "text": clipped(&page_text, MAX_SNAPSHOT_TEXT_CHARS),
    });
    Ok(BrowserToolOutput::Text(bounded_snapshot_json(snapshot)?))
}

pub(crate) async fn click(
    client: &CdpClient,
    handles: &BrowserHandles,
    node_id: &str,
) -> Result<BrowserToolOutput> {
    let backend_node_id = handles.resolve(node_id)?;
    client
        .call(
            "DOM.scrollIntoViewIfNeeded",
            json!({ "backendNodeId": backend_node_id }),
        )
        .await
        .context("scroll target node into view")?;
    let (x, y) = node_center(client, backend_node_id).await?;
    client
        .call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseMoved", "x": x, "y": y, "button": "none" }),
        )
        .await?;
    client
        .call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }),
        )
        .await?;
    client
        .call(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1 }),
        )
        .await?;
    Ok(BrowserToolOutput::Text(format!("clicked {node_id}")))
}

pub(crate) async fn fill(
    client: &CdpClient,
    handles: &BrowserHandles,
    node_id: &str,
    text: &str,
) -> Result<BrowserToolOutput> {
    let backend_node_id = handles.resolve(node_id)?;
    client
        .call("DOM.focus", json!({ "backendNodeId": backend_node_id }))
        .await?;
    key_event::dispatch_select_all(client).await?;
    key_event::dispatch_backspace(client).await?;
    client
        .call("Input.insertText", json!({ "text": text }))
        .await?;
    Ok(BrowserToolOutput::Text(format!("filled {node_id}")))
}

pub(crate) async fn node_center(client: &CdpClient, backend_node_id: u64) -> Result<(f64, f64)> {
    let model = client
        .call(
            "DOM.getBoxModel",
            json!({ "backendNodeId": backend_node_id }),
        )
        .await?;
    let quad = model
        .pointer("/model/content")
        .or_else(|| model.pointer("/model/border"))
        .and_then(Value::as_array)
        .context("node has no visible box model")?;
    anyhow::ensure!(quad.len() == 8, "node box model has an invalid quad");
    let coordinates = quad
        .iter()
        .map(|coordinate| {
            coordinate
                .as_f64()
                .context("node box model coordinate is not numeric")
        })
        .collect::<Result<Vec<_>>>()?;
    let x = coordinates.iter().step_by(2).sum::<f64>() / 4.0;
    let y = coordinates.iter().skip(1).step_by(2).sum::<f64>() / 4.0;
    Ok((x, y))
}

pub(crate) async fn node_is_attached(client: &CdpClient, backend_node_id: u64) -> Result<bool> {
    match client
        .call(
            "DOM.describeNode",
            json!({ "backendNodeId": backend_node_id, "depth": 0 }),
        )
        .await
    {
        Ok(_) => Ok(true),
        Err(error)
            if error.to_string().contains("Could not find node")
                || error.to_string().contains("No node with given id") =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

fn ax_value(node: &Value, field: &str) -> String {
    node.pointer(&format!("/{field}/value"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn ax_property<'a>(node: &'a Value, name: &str) -> Option<&'a Value> {
    node.get("properties")
        .and_then(Value::as_array)?
        .iter()
        .find(|property| property.get("name").and_then(Value::as_str) == Some(name))?
        .pointer("/value/value")
}

fn is_actionable_role(role: &str) -> bool {
    matches!(
        role,
        "button"
            | "checkbox"
            | "combobox"
            | "link"
            | "menuitem"
            | "option"
            | "radio"
            | "searchbox"
            | "slider"
            | "spinbutton"
            | "switch"
            | "tab"
            | "textbox"
            | "treeitem"
    )
}

fn is_editable_role(role: &str) -> bool {
    matches!(role, "combobox" | "searchbox" | "spinbutton" | "textbox")
}

fn clipped(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}
