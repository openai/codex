# UNOVA Oboe Daemon

The UNOVA Oboe Daemon is a monitoring and management service for Codex project components. It provides health checking, status monitoring, and automatic recovery capabilities for critical services.

## Features

- **Service Health Monitoring**: Continuously monitors the health of configured services
- **Automatic Recovery**: Attempts to restart failed services automatically
- **Status Reporting**: Provides detailed status information about all monitored services
- **Configurable**: Supports custom configuration files for different environments
- **Logging**: Comprehensive logging with configurable levels

## Installation

The Oboe daemon is included with the Codex project. No additional installation is required.

## Usage

### Basic Usage

Start the daemon:
```bash
python3 scripts/oboe_daemon.py --daemon
```

Check status:
```bash
python3 scripts/oboe_daemon.py --status
```

Use custom configuration:
```bash
python3 scripts/oboe_daemon.py --config /path/to/config.json --daemon
```

### Configuration

The daemon can be configured using a JSON configuration file. Here's an example:

```json
{
  "monitored_services": ["codex-tui", "codex-mcp-server"],
  "check_interval": 30,
  "max_retries": 3,
  "auto_recovery": true,
  "log_level": "INFO",
  "log_file": "~/.codex/log/oboe-daemon.log"
}
```

#### Configuration Options

- `monitored_services`: List of services to monitor (default: ["codex-tui", "codex-mcp-server"])
- `check_interval`: Time in seconds between health checks (default: 30)
- `max_retries`: Number of failures before attempting recovery (default: 3)
- `auto_recovery`: Whether to automatically attempt service recovery (default: true)
- `log_level`: Logging level (DEBUG, INFO, WARNING, ERROR) (default: "INFO")
- `log_file`: Path to log file (default: "~/.codex/log/oboe-daemon.log")

### Monitored Services

The daemon currently monitors these services:

- **codex-tui**: The Codex Terminal User Interface
- **codex-mcp-server**: The Codex Model Context Protocol server

## Service Recovery

When a service fails health checks repeatedly (exceeding `max_retries`), the daemon will attempt to recover it by:

1. Stopping any existing instances of the service
2. Waiting a brief period for cleanup
3. Optionally restarting the service (depending on the service type)

## Logs

Logs are written to both the console and a log file. The default log location is `~/.codex/log/oboe-daemon.log`. Log levels include:

- **DEBUG**: Detailed information for debugging
- **INFO**: General operational information
- **WARNING**: Warning messages about potential issues
- **ERROR**: Error messages about failures

## Signal Handling

The daemon responds to the following signals:

- **SIGTERM**: Graceful shutdown
- **SIGINT**: Graceful shutdown (Ctrl+C)

## Status Output

The `--status` flag provides detailed information about the daemon and all monitored services:

```json
{
  "daemon": {
    "status": "running",
    "uptime_seconds": 1234,
    "start_time": "2025-09-17T07:47:39.808951"
  },
  "services": {
    "codex-tui": {
      "status": "healthy",
      "last_check": "2025-09-17T07:47:40.123456",
      "consecutive_failures": 0,
      "last_error": null
    },
    "codex-mcp-server": {
      "status": "unhealthy",
      "last_check": "2025-09-17T07:47:40.234567",
      "consecutive_failures": 2,
      "last_error": "Connection refused"
    }
  }
}
```

## Integration with Codex

The Oboe daemon is designed to work seamlessly with the Codex ecosystem. It monitors key components and ensures they remain available for optimal Codex operation.

## Troubleshooting

### Common Issues

1. **Permission denied**: Ensure the script has execute permissions
2. **Module not found**: Verify Python 3 is installed and available
3. **Service not found**: Check that the monitored services are correctly configured
4. **Log file permission**: Ensure the daemon has write access to the log directory

### Debug Mode

For troubleshooting, run with debug logging:

```json
{
  "log_level": "DEBUG"
}
```

This will provide detailed information about health checks and recovery attempts.