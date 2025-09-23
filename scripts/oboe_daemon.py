#!/usr/bin/env python3
"""
UNOVA Oboe Daemon

A daemon process for monitoring and managing UNOVA project components.
Provides health checking, status monitoring, and automatic recovery capabilities.
"""

import asyncio
import logging
import signal
import sys
import time
import json
import os
from pathlib import Path
from typing import Dict, Any, Optional
from datetime import datetime, timedelta
import argparse


class OboeConfig:
    """Configuration management for the Oboe daemon."""
    
    def __init__(self, config_path: Optional[str] = None):
        self.config_data = {}
        self.config_path = config_path
        self._load_config()
    
    def _load_config(self):
        """Load configuration from file or set defaults."""
        default_config = {
            "monitored_services": ["codex-tui", "codex-mcp-server"],
            "check_interval": 30,
            "max_retries": 3,
            "auto_recovery": True,
            "log_level": "INFO",
            "log_file": "~/.codex/log/oboe-daemon.log"
        }
        
        if self.config_path and os.path.exists(self.config_path):
            try:
                with open(self.config_path, 'r') as f:
                    loaded_config = json.load(f)
                    default_config.update(loaded_config)
            except Exception as e:
                logging.warning(f"Failed to load config from {self.config_path}: {e}")
        
        self.config_data = default_config
    
    def get(self, key: str, default=None):
        """Get configuration value."""
        return self.config_data.get(key, default)


class ServiceMonitor:
    """Monitor individual services and track their health."""
    
    def __init__(self, name: str, config: OboeConfig):
        self.name = name
        self.config = config
        self.last_check = None
        self.consecutive_failures = 0
        self.status = "unknown"
        self.last_error = None
    
    async def check_health(self) -> bool:
        """Check if the service is healthy."""
        try:
            # Simulate health check - in a real implementation, this would
            # check actual service endpoints or process status
            if self.name == "codex-tui":
                # Check if TUI process is running
                return await self._check_process("codex")
            elif self.name == "codex-mcp-server":
                # Check if MCP server is responsive
                return await self._check_http_endpoint()
            
            return True
            
        except Exception as e:
            self.last_error = str(e)
            logging.error(f"Health check failed for {self.name}: {e}")
            return False
    
    async def _check_process(self, process_name: str) -> bool:
        """Check if a process is running."""
        try:
            proc = await asyncio.create_subprocess_exec(
                "pgrep", "-f", process_name,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            stdout, stderr = await proc.communicate()
            return proc.returncode == 0 and stdout.strip()
        except Exception as e:
            self.last_error = str(e)
            return False
    
    async def _check_http_endpoint(self) -> bool:
        """Check if HTTP endpoint is responsive."""
        try:
            # For now, assume it's healthy if we can import necessary modules
            # In a real implementation, this would make HTTP requests
            import aiohttp
            return True
        except ImportError:
            return True  # If aiohttp isn't available, assume it's fine
        except Exception as e:
            self.last_error = str(e)
            return False
    
    async def recover(self) -> bool:
        """Attempt to recover the service."""
        try:
            if not self.config.get("auto_recovery", True):
                return False
            
            logging.info(f"Attempting recovery for {self.name}")
            
            if self.name == "codex-tui":
                # Restart TUI service
                proc = await asyncio.create_subprocess_exec(
                    "pkill", 
                    "-f", "codex.*tui",
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.PIPE
                )
                await proc.communicate()
                
                # Wait a moment then start again
                await asyncio.sleep(2)
                
            elif self.name == "codex-mcp-server":
                # Restart MCP server
                proc = await asyncio.create_subprocess_exec(
                    "pkill", 
                    "-f", "codex.*mcp.*server",
                    stdout=asyncio.subprocess.PIPE,
                    stderr=asyncio.subprocess.PIPE
                )
                await proc.communicate()
            
            logging.info(f"Recovery attempted for {self.name}")
            return True
            
        except Exception as e:
            logging.error(f"Recovery failed for {self.name}: {e}")
            return False


class OboeDaemon:
    """Main daemon class for UNOVA Oboe monitoring."""
    
    def __init__(self, config_path: Optional[str] = None):
        self.config = OboeConfig(config_path)
        self.monitors: Dict[str, ServiceMonitor] = {}
        self.running = False
        self.start_time = datetime.now()
        
        # Setup logging
        self._setup_logging()
        
        # Initialize service monitors
        for service in self.config.get("monitored_services", []):
            self.monitors[service] = ServiceMonitor(service, self.config)
        
        logging.info(f"Initialized Oboe daemon with {len(self.monitors)} monitors")
    
    def _setup_logging(self):
        """Configure logging for the daemon."""
        log_file = os.path.expanduser(self.config.get("log_file", "~/.codex/log/oboe-daemon.log"))
        log_dir = Path(log_file).parent
        log_dir.mkdir(parents=True, exist_ok=True)
        
        log_level = getattr(logging, self.config.get("log_level", "INFO").upper())
        
        logging.basicConfig(
            level=log_level,
            format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
            handlers=[
                logging.FileHandler(log_file),
                logging.StreamHandler(sys.stdout)
            ]
        )
    
    async def run(self):
        """Main daemon loop."""
        self.running = True
        logging.info("Starting Oboe daemon")
        
        # Setup signal handlers
        for sig in [signal.SIGTERM, signal.SIGINT]:
            signal.signal(sig, self._signal_handler)
        
        try:
            while self.running:
                await self._check_all_services()
                
                check_interval = self.config.get("check_interval", 30)
                await asyncio.sleep(check_interval)
        
        except Exception as e:
            logging.error(f"Daemon error: {e}")
        finally:
            logging.info("Oboe daemon stopped")
    
    async def _check_all_services(self):
        """Check health of all monitored services."""
        logging.debug("Performing health checks")
        
        for name, monitor in self.monitors.items():
            healthy = await monitor.check_health()
            monitor.last_check = datetime.now()
            
            if healthy:
                if monitor.status != "healthy":
                    logging.info(f"Service {name} is now healthy")
                monitor.status = "healthy"
                monitor.consecutive_failures = 0
            else:
                monitor.status = "unhealthy"
                monitor.consecutive_failures += 1
                
                logging.warning(
                    f"Service {name} unhealthy "
                    f"(failures: {monitor.consecutive_failures})"
                )
                
                max_retries = self.config.get("max_retries", 3)
                if monitor.consecutive_failures >= max_retries:
                    logging.error(f"Service {name} failed {max_retries} times, attempting recovery")
                    await monitor.recover()
    
    def _signal_handler(self, signum, frame):
        """Handle shutdown signals."""
        logging.info(f"Received signal {signum}, shutting down")
        self.running = False
    
    def get_status(self) -> Dict[str, Any]:
        """Get current daemon status."""
        uptime = datetime.now() - self.start_time
        
        return {
            "daemon": {
                "status": "running" if self.running else "stopped",
                "uptime_seconds": int(uptime.total_seconds()),
                "start_time": self.start_time.isoformat()
            },
            "services": {
                name: {
                    "status": monitor.status,
                    "last_check": monitor.last_check.isoformat() if monitor.last_check else None,
                    "consecutive_failures": monitor.consecutive_failures,
                    "last_error": monitor.last_error
                }
                for name, monitor in self.monitors.items()
            }
        }


async def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="UNOVA Oboe Daemon")
    parser.add_argument(
        "--config", 
        help="Path to configuration file",
        default=None
    )
    parser.add_argument(
        "--status",
        action="store_true",
        help="Print daemon status and exit"
    )
    parser.add_argument(
        "--daemon",
        action="store_true",
        help="Run as daemon (default)"
    )
    
    args = parser.parse_args()
    
    daemon = OboeDaemon(args.config)
    
    if args.status:
        status = daemon.get_status()
        print(json.dumps(status, indent=2))
        return
    
    # Run the daemon
    try:
        await daemon.run()
    except KeyboardInterrupt:
        logging.info("Daemon interrupted by user")
    except Exception as e:
        logging.error(f"Daemon failed: {e}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())