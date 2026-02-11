import { TimelineEntry } from "../types";

interface ApprovalCardProps {
  entry: TimelineEntry;
  disabled: boolean;
  onDecision: (requestId: number, decision: "accept" | "acceptForSession" | "decline" | "cancel") => void;
}

export function ApprovalCard({ entry, disabled, onDecision }: ApprovalCardProps) {
  if (typeof entry.requestId !== "number") {
    return null;
  }

  return (
    <div className="approval-card">
      <p className="approval-card__heading">Approval needed</p>
      <pre>{entry.text}</pre>
      <div className="approval-card__actions">
        <button className="solid-btn" disabled={disabled} onClick={() => onDecision(entry.requestId!, "accept")} type="button">
          Accept
        </button>
        <button
          className="ghost-btn"
          disabled={disabled}
          onClick={() => onDecision(entry.requestId!, "acceptForSession")}
          type="button"
        >
          Accept for session
        </button>
        <button
          className="ghost-btn"
          disabled={disabled}
          onClick={() => onDecision(entry.requestId!, "decline")}
          type="button"
        >
          Decline
        </button>
        <button
          className="danger-btn"
          disabled={disabled}
          onClick={() => onDecision(entry.requestId!, "cancel")}
          type="button"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
