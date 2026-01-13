import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import { useAppStore } from "./store";

export type VscodeChatPaneHandle = {
  refreshState: () => void;
};

export const VscodeChatPane = forwardRef<VscodeChatPaneHandle, { rootId: string | null }>(
  function VscodeChatPane(props, ref) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const workspaceSettings = useAppStore((s) => s.workspaceSettings);

  const src = useMemo(() => {
    if (!props.rootId) return null;
    const qp = new URLSearchParams({ rootId: props.rootId });
    return `/webview/chat?${qp.toString()}`;
  }, [props.rootId]);

  useEffect(() => {
    // NOTE: 現状は iframe 内の chat_view_client が完結して動作する前提。
    // openFile/openExternal や root pick などは後続タスクで親へブリッジする。
    void workspaceSettings;
  }, [workspaceSettings]);

  useImperativeHandle(
    ref,
    () => ({
      refreshState: () => {
        const win = iframeRef.current?.contentWindow ?? null;
        if (!win) return;
        win.postMessage({ type: "codexMine.refreshState" }, window.location.origin);
      },
    }),
    [],
  );

  if (!src) return <div className="empty">root がありません。先に Add folder してください。</div>;

  return (
    <iframe
      ref={iframeRef}
      className="chatIframe"
      src={src}
      title="Codex Chat"
      sandbox="allow-scripts allow-forms allow-same-origin"
    />
  );
  },
);
