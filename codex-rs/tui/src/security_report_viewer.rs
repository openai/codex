use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

const REPORT_STYLES: &str = include_str!("security_report_assets/styles.css");
const REPORT_SCRIPT: &str = include_str!("security_report_assets/script.js");
const MARKED_JS: &str = include_str!("security_report_assets/marked.min.js");
const HIGHLIGHT_JS: &str = include_str!("security_report_assets/highlight.min.js");
const MERMAID_JS: &str = include_str!("security_report_assets/mermaid.min.js");

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn build_report_html(title: &str, markdown: &str) -> String {
    let escaped_title = escape_html(title);
    let report_payload = BASE64_STANDARD.encode(markdown);
    let styles = REPORT_STYLES;
    let script = REPORT_SCRIPT;
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{escaped_title}</title>
    <style>{styles}</style>
  </head>
  <body>
    <header class="topbar">
      <div class="brand">
        <div class="site-path" id="site-path">/ Report / {escaped_title}</div>
      </div>
      <div class="top-actions">
        <button id="shareBtn" class="btn primary">Share</button>
        <button id="editToggle" class="btn" aria-pressed="false" title="Toggle edit mode">Edit</button>
        <label class="file-btn btn" for="fileInput">Open</label>
        <input id="fileInput" type="file" accept=".md,.markdown,.txt" hidden />
        <button id="themeToggle" class="icon-btn" title="Toggle dark mode" aria-label="Toggle dark mode">
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path id="themeIcon" fill="currentColor" d="M21.64 13a1 1 0 0 0-1.11-.27 8 8 0 0 1-10.26-10.26 1 1 0 0 0-1.38-1.26 10 10 0 1 0 13 13 1 1 0 0 0-.25-1.21Z"/>
          </svg>
        </button>
      </div>
    </header>

    <div class="drop-overlay" id="dropOverlay" aria-hidden="true">
      <div class="drop-message">
        Drop a .md file to load
      </div>
    </div>

    <main class="layout">
      <article class="content" id="content">
      </article>
      <aside class="sidebar right" id="rightToc" aria-label="Table of contents">
        <div class="toc-inner">
          <div class="nav-title" style="display:flex;align-items:center;justify-content:space-between;gap:8px;">
            <span>Outline</span>
            <button id="navToggle" class="icon-btn nav-toggle" aria-pressed="false" aria-label="Collapse sidebar" title="Collapse sidebar">
              <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                <path id="navIcon" fill="currentColor" d="M9 6l6 6-6 6"/>
              </svg>
            </button>
          </div>
          <div class="toc-search">
            <input id="sectionSearch" class="search-input" type="search" placeholder="Jump to section" aria-label="Jump to section" />
          </div>
          <div id="jobProgressHost"></div>
          <nav id="tocList"></nav>
        </div>
      </aside>
    </main>

    <footer class="footer">
      <div>Drag & drop a Markdown file anywhere, or use Open.</div>
    </footer>

    <script>
      (function() {{
        const base64 = "{report_payload}";
        try {{
          const binary = atob(base64);
          if (typeof TextDecoder === "function") {{
            const bytes = new Uint8Array(binary.length);
            for (let i = 0; i < binary.length; i += 1) {{
              bytes[i] = binary.charCodeAt(i);
            }}
            window.REPORT_MD = new TextDecoder("utf-8").decode(bytes);
          }} else {{
            const percentEncoded = Array.prototype.map
              .call(binary, function (ch) {{
                const code = ch.charCodeAt(0).toString(16).padStart(2, "0");
                return "%" + code;
              }})
              .join("");
            window.REPORT_MD = decodeURIComponent(percentEncoded);
          }}
        }} catch (err) {{
          console.error("Failed to decode embedded report markdown", err);
          window.REPORT_MD = "";
        }}
      }})();
    </script>
    <script>{MARKED_JS}</script>
    <script>{HIGHLIGHT_JS}</script>
    <script>{MERMAID_JS}</script>
    <script>{script}</script>
  </body>
</html>
"#
    )
}
