import { useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import clsx from "clsx";
import { AxiosError } from "axios";
import { fetchActions, executeAction } from "./api";
import { ActionForm } from "./components/ActionForm";
import type { ActionMetadata, ExecuteActionPayload, ExecutionHistoryEntry } from "./types";

function groupByCategory(actions: ActionMetadata[]) {
  return actions.reduce<Record<string, ActionMetadata[]>>((acc, action) => {
    if (!acc[action.category]) {
      acc[action.category] = [];
    }
    acc[action.category].push(action);
    return acc;
  }, {});
}

function formatTimestamp(value: string) {
  const date = new Date(value);
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  }).format(date);
}

export default function App() {
  const { data: actions = [], isLoading, isError } = useQuery({
    queryKey: ["actions"],
    queryFn: fetchActions
  });

  const [selectedActionId, setSelectedActionId] = useState<string | null>(null);
  const [history, setHistory] = useState<ExecutionHistoryEntry[]>([]);

  const selectedAction = useMemo(() => {
    if (!actions.length) {
      return null;
    }
    if (!selectedActionId) {
      return actions[0];
    }
    return actions.find((action) => action.id === selectedActionId) ?? actions[0];
  }, [actions, selectedActionId]);

  const groupedActions = useMemo(() => groupByCategory(actions), [actions]);

  const mutation = useMutation({
    mutationFn: ({ actionId, payload }: { actionId: string; payload: ExecuteActionPayload }) =>
      executeAction(actionId, payload)
  });

  const handleSubmit = (values: Record<string, string>) => {
    if (!selectedAction) {
      return;
    }
    const payload: ExecuteActionPayload = { values };
    const actionSnapshot = selectedAction;
    mutation.mutate(
      { actionId: selectedAction.id, payload },
      {
        onSuccess: (response) => {
          setHistory((prev) => [
            {
              action: actionSnapshot,
              request: payload,
              response
            },
            ...prev
          ]);
        }
      }
    );
  };

  const latestForSelected = useMemo(() => {
    if (!selectedAction) {
      return undefined;
    }
    return history.find((entry) => entry.action.id === selectedAction.id);
  }, [history, selectedAction]);

  const errorMessage = useMemo(() => {
    if (!mutation.isError) {
      return null;
    }
    const error = mutation.error;
    if (error instanceof AxiosError) {
      const data = error.response?.data as { message?: string } | undefined;
      return data?.message ?? error.message;
    }
    return "Failed to run the selected action";
  }, [mutation.error, mutation.isError]);

  if (isLoading) {
    return (
      <div className="app-shell app-shell--loading">
        <p>Loading orchestration actions…</p>
      </div>
    );
  }

  if (isError || !selectedAction) {
    return (
      <div className="app-shell app-shell--error">
        <h1>Codex Control Center</h1>
        <p>We were unable to load the available actions. Please verify that the backend server is running.</p>
      </div>
    );
  }

  const categoryEntries = Object.entries(groupedActions);

  return (
    <div className="app-shell">
      <header className="app-header">
        <div>
          <h1>Codex Control Center</h1>
          <p>
            Launch multi-agent workflows, deep research, and quality gates with a few curated playbooks. Every
            action runs the official Codex CLI under the hood.
          </p>
        </div>
        <div className="app-header__status">
          <span className="status-dot status-dot--online" />
          <span>Backend connected</span>
        </div>
      </header>
      <div className="app-layout">
        <aside className="app-actions">
          {categoryEntries.map(([category, items]) => (
            <section key={category} className="app-actions__section">
              <h2>{category}</h2>
              <div className="app-actions__grid">
                {items.map((action) => (
                  <button
                    key={action.id}
                    className={clsx("action-card", action.id === selectedAction.id && "action-card--active")}
                    onClick={() => setSelectedActionId(action.id)}
                  >
                    <span className="action-card__label">{action.label}</span>
                    <span className="action-card__description">{action.description}</span>
                  </button>
                ))}
              </div>
            </section>
          ))}
        </aside>
        <main className="app-detail">
          <div className="app-detail__summary">
            <h2>{selectedAction.label}</h2>
            <p>{selectedAction.description}</p>
          </div>
          {errorMessage && <div className="app-detail__error">{errorMessage}</div>}
          <ActionForm action={selectedAction} onSubmit={handleSubmit} isSubmitting={mutation.isPending} />
          {latestForSelected && <ExecutionSummary entry={latestForSelected} />}
        </main>
        <aside className="app-history">
          <div className="app-history__header">
            <h2>Run history</h2>
            <span>{history.length} run{history.length === 1 ? "" : "s"}</span>
          </div>
          {history.length === 0 ? (
            <p className="app-history__empty">No runs yet. Trigger an action to see structured results here.</p>
          ) : (
            <ul className="app-history__list">
              {history.map((entry) => (
                <HistoryItem key={entry.response.id} entry={entry} />
              ))}
            </ul>
          )}
        </aside>
      </div>
    </div>
  );
}

function ExecutionSummary({ entry }: { entry: ExecutionHistoryEntry }) {
  const { action, response } = entry;
  const succeeded = response.status === "completed";
  const commandLine = response.command.join(" ");
  const hasStdout = response.stdout.length > 0;
  const hasStderr = response.stderr.length > 0;

  return (
    <section className="execution-card">
      <header className="execution-card__header">
        <div>
          <h3>Latest run</h3>
          <p>
            {action.label} · {formatTimestamp(response.executedAt)}
          </p>
        </div>
        <span className={clsx("status-badge", succeeded ? "status-badge--success" : "status-badge--failure")}>
          {succeeded ? "Completed" : "Failed"}
        </span>
      </header>
      <div className="execution-card__command" title={commandLine}>
        <code>{commandLine}</code>
      </div>
      <div className="execution-card__meta">
        <span>Exit code: {response.exitCode ?? "N/A"}</span>
        <span>Duration: {response.durationMs} ms</span>
      </div>
      {hasStdout && (
        <section className="execution-card__output">
          <h4>stdout</h4>
          <pre>{response.stdout}</pre>
        </section>
      )}
      {hasStderr && (
        <section className="execution-card__output">
          <h4>stderr</h4>
          <pre>{response.stderr}</pre>
        </section>
      )}
    </section>
  );
}

function HistoryItem({ entry }: { entry: ExecutionHistoryEntry }) {
  const { action, request, response } = entry;
  const succeeded = response.status === "completed";
  const summary = request.values[Object.keys(request.values)[0]];

  return (
    <li className="history-item">
      <div className="history-item__header">
        <span className="history-item__title">{action.label}</span>
        <span className={clsx("status-dot", succeeded ? "status-dot--success" : "status-dot--error")} />
      </div>
      <p className="history-item__timestamp">{formatTimestamp(response.executedAt)}</p>
      {summary && <p className="history-item__summary">{summary}</p>}
    </li>
  );
}
