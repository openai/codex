import { useMemo, useState } from "react";
import { formatTimestamp } from "../lib/time";
import { ThreadSummary } from "../types";

interface SidebarProps {
  open: boolean;
  threads: ThreadSummary[];
  threadsLoaded: boolean;
  nextCursor: string | null;
  loadingMore: boolean;
  activeThreadId: string | null;
  onRefresh: () => void;
  onLoadMore: () => void;
  onNewThread: () => void;
  onResumeThread: (id: string) => void;
  onDismissMobile: () => void;
}

export function Sidebar({
  open,
  threads,
  threadsLoaded,
  nextCursor,
  loadingMore,
  activeThreadId,
  onRefresh,
  onLoadMore,
  onNewThread,
  onResumeThread,
  onDismissMobile,
}: SidebarProps) {
  const [searchText, setSearchText] = useState("");
  const normalizedSearchText = searchText.trim().toLowerCase();
  const filteredThreads = useMemo(() => {
    if (normalizedSearchText.length === 0) {
      return threads;
    }

    return threads.filter((thread) => thread.searchText.includes(normalizedSearchText));
  }, [normalizedSearchText, threads]);

  return (
    <aside className={`sidebar ${open ? "sidebar--open" : ""}`}>
      <div className="sidebar__header">
        <p>Sessions</p>
        <div className="stacked-actions">
          <button className="ghost-btn" onClick={onRefresh} type="button">
            Refresh
          </button>
          <button className="solid-btn" onClick={onNewThread} type="button">
            New session
          </button>
        </div>
      </div>

      <div className="session-search">
        <input
          onChange={(event) => setSearchText(event.target.value)}
          placeholder="Search sessions"
          type="search"
          value={searchText}
        />
      </div>

      <div
        className="thread-list"
        onScroll={(event) => {
          if (!nextCursor || loadingMore) {
            return;
          }
          const target = event.currentTarget;
          const distanceToBottom = target.scrollHeight - target.clientHeight - target.scrollTop;
          if (distanceToBottom < 120) {
            onLoadMore();
          }
        }}
      >
        {!threadsLoaded ? <p className="empty-note">Loading sessions...</p> : null}
        {threadsLoaded && threads.length === 0 ? <p className="empty-note">No sessions yet.</p> : null}
        {threadsLoaded && threads.length > 0 && filteredThreads.length === 0 ? (
          <p className="empty-note">No sessions match your search.</p>
        ) : null}

        {filteredThreads.map((thread) => (
          <button
            className={`thread-card ${activeThreadId === thread.id ? "thread-card--active" : ""}`}
            key={thread.id}
            onClick={() => {
              onResumeThread(thread.id);
              onDismissMobile();
            }}
            type="button"
          >
            <p className="thread-card__title" title={thread.title}>
              {thread.title}
            </p>
            {thread.preview.length > 0 && thread.preview !== thread.title ? (
              <p className="thread-card__preview">{thread.preview}</p>
            ) : null}
            <div className="thread-card__meta">
              <span>{formatTimestamp(thread.updatedAt) || "No messages yet"}</span>
            </div>
          </button>
        ))}

        {threadsLoaded && nextCursor ? (
          <button className="ghost-btn load-more-btn" disabled={loadingMore} onClick={onLoadMore} type="button">
            {loadingMore ? "Loading more…" : "Load more sessions"}
          </button>
        ) : null}
      </div>
    </aside>
  );
}
