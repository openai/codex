use anyhow::Result;

pub(crate) const SNAPSHOT_EXPRESSION: &str = r#"
(() => {
  const MAX_NODES = 100;
  const MAX_TEXT = 12000;
  const selector = [
    'a[href]', 'button', 'input', 'textarea', 'select', 'summary',
    '[role="button"]', '[role="link"]', '[role="checkbox"]',
    '[role="menuitem"]', '[role="option"]', '[role="radio"]',
    '[role="switch"]', '[tabindex]', '[contenteditable="true"]'
  ].join(',');
  if (typeof document.__codexBrowserDocumentId !== 'string') {
    const random = new Uint32Array(2);
    globalThis.crypto.getRandomValues(random);
    document.__codexBrowserDocumentId = Array.from(
      random,
      value => value.toString(16).padStart(8, '0')
    ).join('');
  }
  if (!Number.isSafeInteger(document.__codexBrowserNextId) || document.__codexBrowserNextId < 1) {
    document.__codexBrowserNextId = 1;
  }
  if (!(document.__codexBrowserNodeIds instanceof WeakMap)) {
    document.__codexBrowserNodeIds = new WeakMap();
  }
  const nodeMap = new Map();
  document.__codexBrowserNodes = nodeMap;
  const visible = (element) => {
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.visibility !== 'hidden' && style.display !== 'none' &&
      rect.width > 0 && rect.height > 0;
  };
  const clipped = (value, limit = 200) => String(value || '').trim().slice(0, limit);
  const nodes = [];
  for (const element of document.querySelectorAll(selector)) {
    if (nodes.length >= MAX_NODES || !visible(element)) continue;
    let nodeId = document.__codexBrowserNodeIds.get(element);
    if (!nodeId) {
      nodeId = `d${document.__codexBrowserDocumentId}n${document.__codexBrowserNextId++}`;
      document.__codexBrowserNodeIds.set(element, nodeId);
    }
    nodeMap.set(nodeId, element);
    const isPassword = element instanceof HTMLInputElement && element.type === 'password';
    const value = 'value' in element ? (isPassword ? '<redacted>' : element.value) : '';
    nodes.push({
      nodeId,
      tag: element.tagName.toLowerCase(),
      role: clipped(element.getAttribute('role')),
      name: clipped(element.getAttribute('aria-label') || element.getAttribute('name') ||
        element.getAttribute('alt') || element.getAttribute('title')),
      text: clipped(element.innerText || element.textContent),
      value: clipped(value),
      disabled: Boolean(element.disabled),
      checked: 'checked' in element ? Boolean(element.checked) : undefined
    });
  }
  return {
    url: location.href,
    title: document.title,
    viewport: { width: innerWidth, height: innerHeight, scrollX, scrollY },
    nodes,
    text: clipped(document.body?.innerText, MAX_TEXT)
  };
})()
"#;

pub(crate) fn click_expression(node_id: &str) -> Result<String> {
    let node_id = node_id_literal(node_id)?;
    Ok(format!(
        r#"(() => {{
  const element = document.__codexBrowserNodes?.get({node_id});
  if (!element || !element.isConnected) return {{ ok: false, error: 'node_not_found' }};
  element.scrollIntoView({{ block: 'center', inline: 'center' }});
  element.click();
  return {{ ok: true }};
}})()"#
    ))
}

pub(crate) fn fill_expression(node_id: &str, text: &str) -> Result<String> {
    let node_id = node_id_literal(node_id)?;
    let text = serde_json::to_string(text)?;
    Ok(format!(
        r#"(() => {{
  const element = document.__codexBrowserNodes?.get({node_id});
  if (!element || !element.isConnected) return {{ ok: false, error: 'node_not_found' }};
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {{
    const prototype = element instanceof HTMLInputElement ?
      HTMLInputElement.prototype : HTMLTextAreaElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
    if (!setter) return {{ ok: false, error: 'node_not_fillable' }};
    setter.call(element, {text});
  }} else if (element instanceof HTMLSelectElement) {{
    element.value = {text};
  }} else if (element.isContentEditable) {{
    element.textContent = {text};
  }} else {{
    return {{ ok: false, error: 'node_not_fillable' }};
  }}
  element.focus();
  element.dispatchEvent(new Event('input', {{ bubbles: true }}));
  element.dispatchEvent(new Event('change', {{ bubbles: true }}));
  return {{ ok: true }};
}})()"#
    ))
}

pub(crate) fn scroll_expression(delta_x: i64, delta_y: i64) -> String {
    format!("(() => {{ window.scrollBy({delta_x}, {delta_y}); return {{ scrollX, scrollY }}; }})()")
}

pub(crate) fn key_code(key: &str) -> &str {
    match key {
        "Enter" => "Enter",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "Backspace",
        "Delete" => "Delete",
        "ArrowUp" => "ArrowUp",
        "ArrowDown" => "ArrowDown",
        "ArrowLeft" => "ArrowLeft",
        "ArrowRight" => "ArrowRight",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PageUp",
        "PageDown" => "PageDown",
        _ => key,
    }
}

fn node_id_literal(node_id: &str) -> Result<String> {
    let Some((document_id, node_number)) =
        node_id.strip_prefix('d').and_then(|id| id.split_once('n'))
    else {
        anyhow::bail!("invalid nodeId; take a new snapshot and use its nodeId");
    };
    let valid_document_id = document_id.len() == 16
        && document_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let valid_node_number = !node_number.is_empty()
        && node_number.len() <= 20
        && node_number.bytes().all(|byte| byte.is_ascii_digit());
    anyhow::ensure!(
        valid_document_id && valid_node_number,
        "invalid nodeId; take a new snapshot and use its nodeId"
    );
    Ok(serde_json::to_string(node_id)?)
}
