import "../styles/RecentChanges.css";

interface Change {
  id: number;
  timestamp: string;
  file_path: string;
  change_type: string;
  diff_lines_added: number;
  diff_lines_removed: number;
}

interface RecentChangesProps {
  changes: Change[];
}

function RecentChanges({ changes }: RecentChangesProps) {
  const getChangeTypeIcon = (type: string) => {
    switch (type) {
      case "Created":
        return "✨";
      case "Modified":
        return "✏️";
      case "Deleted":
        return "🗑️";
      default:
        return "📄";
    }
  };

  const formatTimestamp = (timestamp: string) => {
    try {
      const date = new Date(timestamp);
      return date.toLocaleString();
    } catch {
      return timestamp;
    }
  };

  return (
    <div className="recent-changes">
      <h2>Recent Changes</h2>
      
      {changes.length === 0 ? (
        <p className="no-changes">No changes yet. Start monitoring a workspace to see file changes here.</p>
      ) : (
        <div className="changes-list">
          {changes.map((change) => (
            <div key={change.id} className="change-item">
              <div className="change-icon">
                {getChangeTypeIcon(change.change_type)}
              </div>
              <div className="change-info">
                <div className="change-header">
                  <span className="change-path">{change.file_path}</span>
                  <span className="change-type">{change.change_type}</span>
                </div>
                <div className="change-details">
                  <span className="change-timestamp">{formatTimestamp(change.timestamp)}</span>
                  <span className="change-diff">
                    <span className="diff-added">+{change.diff_lines_added}</span>
                    <span className="diff-removed">-{change.diff_lines_removed}</span>
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default RecentChanges;

