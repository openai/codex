import { useEffect, useMemo, useRef, useState } from "react";
import { renderMarkdown } from "../lib/markdown";
import { TimelineEntry } from "../types";
import { ApprovalCard } from "./ApprovalCard";

interface ChatFeedProps {
  entries: TimelineEntry[];
  activeApprovals: Record<number, true>;
  onApprovalDecision: (
    requestId: number,
    decision: "accept" | "acceptForSession" | "decline" | "cancel",
  ) => void;
}

export function ChatFeed({ entries, activeApprovals, onApprovalDecision }: ChatFeedProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [stickToBottom, setStickToBottom] = useState(true);

  useEffect(() => {
    if (!stickToBottom) {
      return;
    }
    if (!scrollRef.current) {
      return;
    }
    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [entries, stickToBottom]);

  const groups = useMemo(() => {
    const out: TimelineEntry[][] = [];
    for (const entry of entries) {
      const previous = out.at(-1);
      if (
        previous &&
        previous[0].kind === entry.kind &&
        entry.kind !== "approval" &&
        entry.kind !== "system" &&
        entry.kind !== "notification"
      ) {
        previous.push(entry);
      } else {
        out.push([entry]);
      }
    }
    return out;
  }, [entries]);

  return (
    <section
      className="chat-feed"
      onScroll={(event) => {
        const target = event.currentTarget;
        const bottomGap = target.scrollHeight - target.clientHeight - target.scrollTop;
        setStickToBottom(bottomGap < 48);
      }}
      ref={scrollRef}
    >
      {groups.length === 0 ? <p className="empty-note">Start a session and send a prompt to begin.</p> : null}

      {groups.map((group) => {
        const representative = group[0];

        return (
          <article className={`entry entry--${representative.kind}`} key={representative.key}>
            <header>
              <span>{representative.label}</span>
              {representative.status ? <span className="entry-status">{representative.status}</span> : null}
            </header>

            {group.map((entry) => {
              if (entry.kind === "approval") {
                return (
                  <ApprovalCard
                    disabled={!activeApprovals[entry.requestId ?? -1]}
                    entry={entry}
                    key={entry.key}
                    onDecision={onApprovalDecision}
                  />
                );
              }

              if (entry.kind === "assistant" || entry.kind === "reasoning") {
                return (
                  <div
                    className="entry-markdown"
                    dangerouslySetInnerHTML={{ __html: renderMarkdown(entry.text) }}
                    key={entry.key}
                  />
                );
              }

              if (entry.kind === "command" || entry.kind === "file-change") {
                return (
                  <pre className="entry-pre" key={entry.key}>
                    {entry.text}
                  </pre>
                );
              }

              return (
                <p className="entry-text" key={entry.key}>
                  {entry.text}
                </p>
              );
            })}

            {representative.meta ? <p className="entry-meta">{representative.meta}</p> : null}
          </article>
        );
      })}
    </section>
  );
}
