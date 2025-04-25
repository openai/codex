#!/usr/bin/env python3
"""
Basic Predator Mode multi-agent orchestrator:
  - Invokes Recon agent to collect intel
  - Optionally runs nmap if open_ports missing
  - Merges findings into state.yaml
  - Invokes Exploit agent with shared state
"""
import os
import subprocess
import json
try:
    import yaml
    load_yaml = yaml.safe_load
    dump_yaml = lambda obj, f: yaml.safe_dump(obj, f)
except ImportError:
    load_yaml = json.load
    dump_yaml = lambda obj, f: json.dump(obj, f, indent=2)
import shutil
import logging
from datetime import datetime

TARGET = "https://ginandjuice.shop"
STATE_FILE = "state.yaml"
# Ensure logs directory exists
os.makedirs('logs', exist_ok=True)
# Configure structured logging to file
logging.basicConfig(
    filename='logs/agent-history.log',
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s'
)
"""
# Load tool registry if available (YAML or JSON)
"""
try:
    with open('tools.yaml') as tf:
        tools = load_yaml(tf).get('tools', {})
except Exception:
    tools = {}

def main():
    # Load existing state
    if os.path.exists(STATE_FILE):
        state = load_yaml(open(STATE_FILE))
    else:
        logging.error(f"Missing {STATE_FILE}, aborting orchestration.")
        return
    # Iterate through OWASP phases and orchestrate fallbacks
    MAX_RETRIES = 3
    registry = tools
    def run_tool(name, cmd_tmpl, phase, stage):
        out = subprocess.getoutput(cmd_tmpl.replace('{target}', state.get('target', '')))
        success = bool(out.strip())
        entry = {
            'phase': phase,
            'stage': stage,
            'tool': name,
            'cmd': cmd_tmpl.replace('{target}', state.get('target', '')),
            'success': success,
            'output': out,
            'timestamp': datetime.utcnow().isoformat()
        }
        state.setdefault('steps', []).append(entry)
        with open(STATE_FILE, 'w') as f:
            dump_yaml(state, f)
        logging.info(f"Tool {name} ({stage}) for {phase} -> success={success}")
        return success
    # Orchestrate each vuln
    phases = state.get('owasp_phases', OWASP_TOP10)
    for phase in phases:
        # Detect
        dets = [s for s in state.get('steps', []) if s['phase']==phase and s['stage']=='detect' and s['success']]
        if not dets:
            for name, info in registry.items():
                if info.get('type')!='recon': continue
                for _ in range(MAX_RETRIES):
                    if run_tool(name, info['command'], phase, 'detect'): break
                else: continue
                break
        # Exploit
        if any(s['success'] for s in state.get('steps', []) if s['phase']==phase and s['stage']=='detect'):
            exs = [s for s in state.get('steps', []) if s['phase']==phase and s['stage']=='exploit' and s['success']]
            if not exs:
                for name, info in registry.items():
                    if info.get('type')!='exploit': continue
                    for _ in range(MAX_RETRIES):
                        if run_tool(name, info['command'], phase, 'exploit'): break
                    else: continue
                    break
    # Final summary
    report = {'summary': {}, 'steps': state.get('steps', [])}
    for phase in phases:
        report['summary'][phase] = {
            'detected': any(s['phase']==phase and s['stage']=='detect' and s['success'] for s in report['steps']),
            'exploited': any(s['phase']==phase and s['stage']=='exploit' and s['success'] for s in report['steps']),
        }
    with open('mission_report.yaml','w') as f:
        yaml.safe_dump(report, f)
    logging.info("Mission report written to mission_report.yaml")

if __name__ == '__main__':
    main()