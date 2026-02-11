import { AppAction, AppState, TimelineEntry } from "../types";

function upsertTimeline(timeline: TimelineEntry[], entry: TimelineEntry): TimelineEntry[] {
  const index = timeline.findIndex((item) => item.key === entry.key);
  if (index === -1) {
    return [...timeline, entry];
  }

  const next = [...timeline];
  next[index] = {
    ...next[index],
    ...entry,
    createdAt: next[index].createdAt,
  };
  return next;
}

export const initialState: AppState = {
  connection: "connecting",
  statusText: "connecting",
  initialized: false,
  threadId: null,
  threads: [],
  threadsLoaded: false,
  threadsNextCursor: null,
  threadsLoadingMore: false,
  timeline: [],
  itemToTimeline: {},
  activeApprovals: {},
  logs: [],
  logPanelOpen: false,
  sidebarOpen: false,
  streamActive: false,
  theme: window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark",
};

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case "set_connection":
      return {
        ...state,
        connection: action.connection,
        statusText: action.statusText,
      };

    case "set_initialized":
      return {
        ...state,
        initialized: action.initialized,
      };

    case "set_thread":
      return {
        ...state,
        threadId: action.threadId,
      };

    case "set_threads":
      return {
        ...state,
        threads: action.threads,
        threadsLoaded: true,
        threadsNextCursor: action.nextCursor,
        threadsLoadingMore: false,
      };

    case "append_threads": {
      const existingIds = new Set(state.threads.map((thread) => thread.id));
      const appended = [...state.threads];
      for (const thread of action.threads) {
        if (existingIds.has(thread.id)) {
          continue;
        }
        existingIds.add(thread.id);
        appended.push(thread);
      }

      return {
        ...state,
        threads: appended,
        threadsLoaded: true,
        threadsNextCursor: action.nextCursor,
        threadsLoadingMore: false,
      };
    }

    case "set_threads_loading_more":
      return {
        ...state,
        threadsLoadingMore: action.loading,
      };

    case "clear_timeline":
      return {
        ...state,
        timeline: [],
        itemToTimeline: {},
        activeApprovals: {},
        streamActive: false,
      };

    case "upsert_entry":
      return {
        ...state,
        timeline: upsertTimeline(state.timeline, action.entry),
      };

    case "append_delta": {
      const key = state.itemToTimeline[action.itemId] ?? `item:${action.itemId}`;
      const existing = state.timeline.find((entry) => entry.key === key);
      const currentText = existing?.text ?? "";
      return {
        ...state,
        itemToTimeline: {
          ...state.itemToTimeline,
          [action.itemId]: key,
        },
        timeline: upsertTimeline(state.timeline, {
          key,
          kind: existing?.kind ?? "assistant",
          label: existing?.label ?? "Assistant",
          text: `${currentText}${action.delta}`,
          status: existing?.status,
          meta: existing?.meta,
          createdAt: existing?.createdAt ?? Date.now(),
        }),
      };
    }

    case "set_item_key":
      return {
        ...state,
        itemToTimeline: {
          ...state.itemToTimeline,
          [action.itemId]: action.key,
        },
      };

    case "activate_approval":
      return {
        ...state,
        activeApprovals: {
          ...state.activeApprovals,
          [action.requestId]: true,
        },
      };

    case "complete_approval":
      return {
        ...state,
        activeApprovals: Object.fromEntries(
          Object.entries(state.activeApprovals).filter(([id]) => Number(id) !== action.requestId),
        ),
        timeline: state.timeline.map((entry) => {
          if (entry.requestId !== action.requestId) {
            return entry;
          }
          return {
            ...entry,
            status: action.status,
          };
        }),
      };

    case "set_log_panel":
      return {
        ...state,
        logPanelOpen: action.open,
      };

    case "append_log": {
      const logs = [action.log, ...state.logs].slice(0, 600);
      return {
        ...state,
        logs,
      };
    }

    case "set_sidebar":
      return {
        ...state,
        sidebarOpen: action.open,
      };

    case "set_stream_active":
      return {
        ...state,
        streamActive: action.active,
      };

    case "set_theme":
      return {
        ...state,
        theme: action.theme,
      };

    default:
      return state;
  }
}
