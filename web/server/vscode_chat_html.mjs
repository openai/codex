export function renderVscodeChatHtml(args) {
  const clientScriptSrc = String(args.clientScriptSrc ?? "");
  const markdownItSrc = String(args.markdownItSrc ?? "");
  const shimScript = String(args.shimScript ?? "");
  const title = String(args.title ?? "Codex UI");

  if (!clientScriptSrc) throw new Error("clientScriptSrc is required");
  if (!markdownItSrc) throw new Error("markdownItSrc is required");

  // NOTE: This HTML intentionally mirrors the VSCode extension's webview DOM shape
  // expected by `vscode-extension/src/ui/chat_view_client.ts`.
  return `<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${escapeHtml(title)}</title>
    <style>
      :root {
        --cm-font-family: var(--vscode-font-family, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif);
        --cm-font-size: var(--vscode-font-size, 13px);
        --cm-font-weight: var(--vscode-font-weight, 400);
        --cm-editor-font-family: var(--vscode-editor-font-family, var(--cm-font-family));
        --cm-editor-font-size: var(--vscode-editor-font-size, var(--cm-font-size));
        --cm-editor-font-weight: var(--vscode-editor-font-weight, var(--cm-font-weight));
        --cm-line-height: 1.55;
        --cm-chat-image-max-height: 360px;
      }

      body { font-family: var(--cm-font-family); font-size: var(--cm-font-size); font-weight: var(--cm-font-weight); line-height: var(--cm-line-height); -webkit-font-smoothing: antialiased; margin: 0; padding: 0; height: 100vh; display: flex; flex-direction: column; overflow-x: hidden; background: var(--vscode-editor-background, #1e1e1e); color: var(--vscode-editor-foreground, #d4d4d4); }
      .top { padding: 10px 12px; border-bottom: 1px solid rgba(127,127,127,0.3); display: flex; flex-direction: column; gap: 8px; }
      .topRow { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
      .title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
      .actions { display: flex; gap: 8px; }
      button { padding: 6px 10px; border-radius: 6px; border: 1px solid rgba(127,127,127,0.35); background: transparent; color: inherit; cursor: pointer; }
      .actions button { font-size: 12px; padding: 4px 8px; }
      button:disabled { opacity: 0.5; cursor: default; }
      button.iconBtn { width: 32px; height: 28px; padding: 0; display: inline-flex; align-items: center; justify-content: center; }
      button.iconBtn::before { content: "↑"; font-size: 16px; transform: rotate(-90deg); display: inline-block; opacity: 0.9; }
      button.iconBtn[data-mode="stop"]::before { content: "■"; transform: none; font-size: 14px; }
      button.iconBtn.settingsBtn::before { content: "⚙"; transform: none; font-size: 14px; }
      .tabs { display: flex; gap: 8px; overflow-x: auto; padding-bottom: 2px; flex-wrap: wrap; min-height: 34px; }
      .tabGroup { display: inline-flex; align-items: center; gap: 8px; border: 1px solid rgba(127,127,127,0.25); border-radius: 14px; padding: 6px 8px; background: rgba(127,127,127,0.04); }
      .tabGroupLabel { padding: 4px 8px; border-radius: 999px; border: 1px solid rgba(14,99,156,0.55); color: rgba(220,220,220,0.95); font-size: 12px; white-space: nowrap; }
      .tabGroupTabs { display: inline-flex; align-items: center; gap: 6px; overflow-x: auto; }
      .tab { display: inline-flex; align-items: center; gap: 8px; padding: 6px 10px; border-radius: 999px; border: 1px solid rgba(127,127,127,0.35); cursor: pointer; user-select: none; font-size: 12px; white-space: nowrap; }
      .tab.active { background: rgba(127,127,127,0.18); border-color: rgba(127,127,127,0.55); }
      .tab.running { border-color: rgba(46,160,67,0.55); background: rgba(46,160,67,0.10); }
      .tab.unread { border-color: rgba(210,153,34,0.55); background: rgba(210,153,34,0.10); }
      .tab .close { margin-left: 2px; opacity: 0.7; }
      .approvals { padding: 10px 12px; border-bottom: 1px solid rgba(127,127,127,0.25); }
      .approval { border: 1px solid rgba(127,127,127,0.35); border-radius: 10px; padding: 10px 12px; background: rgba(0,0,0,0.03); }
      .approvalTitle { font-weight: 600; margin-bottom: 8px; }
      .approvalActions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
      .log { flex: 1; min-height: 0; overflow: auto; padding: 10px 12px 18px; }
      .msg { border: 1px solid rgba(127,127,127,0.25); border-radius: 12px; padding: 10px 12px; margin-bottom: 10px; background: rgba(0,0,0,0.02); }
      .user { border-color: rgba(0,120,212,0.25); background: rgba(0,120,212,0.06); }
      .assistant { border-color: rgba(127,127,127,0.25); }
      .divider { border-color: rgba(127,127,127,0.18); background: rgba(127,127,127,0.06); opacity: 0.95; }
      .note { border-color: rgba(127,127,127,0.18); background: rgba(127,127,127,0.04); }
      pre { margin: 0; white-space: pre-wrap; word-break: break-word; font-family: var(--cm-editor-font-family); font-size: var(--cm-editor-font-size); font-weight: var(--cm-editor-font-weight); line-height: var(--cm-line-height); }
      .composer { border-top: 1px solid rgba(127,127,127,0.3); padding: 10px 12px; display: flex; flex-direction: column; gap: 8px; position: relative; padding-bottom: calc(10px + env(safe-area-inset-bottom)); }
      .inputRow { display: flex; gap: 8px; align-items: flex-end; }
      textarea { flex: 1; resize: none; box-sizing: border-box; border-radius: 8px; border: 1px solid rgba(127,127,127,0.35); padding: 6px 10px; background: transparent; color: inherit; font-family: var(--cm-editor-font-family); font-size: var(--cm-editor-font-size); font-weight: var(--cm-editor-font-weight); line-height: 1.2; overflow-y: hidden; min-height: 30px; max-height: 200px; }
      textarea::placeholder { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
      .suggest { position: absolute; left: 12px; right: 12px; bottom: calc(100% + 8px); border: 1px solid rgba(127,127,127,0.35); border-radius: 10px; background: rgba(30,30,30,0.95); max-height: 160px; overflow: auto; display: none; z-index: 20; box-shadow: 0 8px 24px rgba(0,0,0,0.35); }
      .requestUserInput { display: none; padding: 10px 12px; border-top: 1px solid rgba(127,127,127,0.25); border-bottom: 1px solid rgba(127,127,127,0.25); }
      .footerBar { padding: 8px 12px; border-top: 1px solid rgba(127,127,127,0.25); display: none; }
      .footerStatus { font-size: 12px; opacity: 0.75; white-space: pre-wrap; word-break: break-word; }
      .toast { position: fixed; top: 16px; left: 50%; transform: translateX(-50%); z-index: 1000; max-width: min(820px, calc(100vw - 32px)); border-radius: 10px; padding: 10px 12px; border: 1px solid rgba(127,127,127,0.35); box-shadow: 0 10px 30px rgba(0,0,0,0.35); background: rgba(30,30,30,0.95); display: none; }
      .toast.success { border-color: rgba(0,200,120,0.55); }
      .toast.error { border-color: rgba(220,60,60,0.60); }
      @media (max-width: 820px) {
        .top { padding-top: calc(10px + env(safe-area-inset-top)); }
        .actions button { padding: 8px 10px; }
        button.iconBtn { width: 40px; height: 36px; }
        .tabs { flex-wrap: nowrap; min-height: 40px; }
      }
    </style>
  </head>
  <body>
    <div class="top">
      <div class="topRow">
        <div id="title" class="title">${escapeHtml(title)}</div>
        <div class="actions">
          <button id="new">New</button>
          <button id="resume">Resume</button>
          <button id="reload" title="Reload session (codez only)" disabled>Reload</button>
          <button id="settings" class="iconBtn settingsBtn" aria-label="Settings" title="Settings"></button>
        </div>
      </div>
      <div id="tabs" class="tabs"></div>
    </div>
    <div id="approvals" class="approvals" style="display:none"></div>
    <div id="log" class="log"></div>
    <div id="composer" class="composer">
      <div id="editBanner" class="editBanner" style="display:none"></div>
      <div id="requestUserInput" class="requestUserInput"></div>
      <div id="attachments" class="attachments"></div>
      <button id="returnToBottom" class="returnToBottomBtn" title="Scroll to bottom">Return to Bottom</button>
      <div id="inputRow" class="inputRow">
        <input id="imageInput" type="file" accept="image/*" multiple style="display:none" />
        <button id="attach" class="iconBtn attachBtn" aria-label="Attach image" title="Attach image"></button>
        <textarea id="input" rows="1" placeholder="Type a message"></textarea>
        <button id="send" class="iconBtn" data-mode="send" aria-label="Send" title="Send (Esc: stop)"></button>
      </div>
      <div id="suggest" class="suggest"></div>
    </div>
    <div class="footerBar">
      <div id="modelBar" class="modelBar"></div>
      <div id="statusText" class="footerStatus" style="display:none"></div>
      <div id="statusPopover" class="statusPopover" style="display:none"></div>
    </div>
    <div id="toast" class="toast"></div>
    <script>${shimScript}</script>
    <script src="${escapeAttr(markdownItSrc)}"></script>
    <script src="${escapeAttr(clientScriptSrc)}"></script>
  </body>
</html>`;
}

function escapeHtml(s) {
  return String(s)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function escapeAttr(s) {
  // keep it simple: treat as URL-ish and escape quotes/angles
  return escapeHtml(s);
}
