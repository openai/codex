#!/usr/bin/env python3
"""Convert common Unix commands to Windows equivalents.

Reads a JSON object from stdin with a `command` array and emits a JSON object
with the possibly adapted command and a boolean `adapted` flag.
"""
import json
import sys

COMMAND_MAP = {
    "ls": "dir",
    "grep": "findstr",
    "cat": "type",
    "rm": "del",
    "cp": "copy",
    "mv": "move",
    "touch": "echo.>",
    "mkdir": "md",
}

OPTION_MAP = {
    "ls": {"-l": "/p", "-a": "/a", "-R": "/s"},
    "grep": {"-i": "/i", "-r": "/s"},
}

def adapt(cmd):
    if not cmd:
        return cmd, False
    prog = cmd[0]
    if prog == "pwd":
        return ["cmd", "/c", "cd"], True
    if prog in ("env", "printenv"):
        return ["cmd", "/c", "set"], True
    if prog not in COMMAND_MAP:
        return cmd, False
    new_cmd = [COMMAND_MAP[prog]] + cmd[1:]
    if prog in OPTION_MAP:
        opts = OPTION_MAP[prog]
        new_cmd = [new_cmd[0]] + [opts.get(arg, arg) for arg in cmd[1:]]
    return new_cmd, True

def main():
    try:
        data = json.loads(sys.stdin.read() or "{}")
        cmd = data.get("command", [])
    except json.JSONDecodeError:
        cmd = []
    adapted_cmd, changed = adapt(cmd)
    sys.stdout.write(json.dumps({"command": adapted_cmd, "adapted": changed}))

if __name__ == "__main__":
    main()
