import "../styles/StatusCard.css";

interface StatusCardProps {
  title: string;
  status: string;
  icon: string;
}

function StatusCard({ title, status, icon }: StatusCardProps) {
  const getStatusColor = () => {
    if (status === "running" || status.startsWith("0.")) {
      return "status-success";
    } else if (status === "stopped" || status === "unknown") {
      return "status-warning";
    }
    return "status-info";
  };

  return (
    <div className="status-card">
      <div className="status-icon">{icon}</div>
      <div className="status-info">
        <h3>{title}</h3>
        <p className={`status-text ${getStatusColor()}`}>{status}</p>
      </div>
    </div>
  );
}

export default StatusCard;

