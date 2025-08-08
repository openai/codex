import json
import os
import platform
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
    "ls": {
        "-l": "/p",
        "-a": "/a",
        "-R": "/s",
    },
    "grep": {
        "-i": "/i",
        "-r": "/s",
    },
}

def adapt(cmd):
    if not cmd:
        return cmd, ""
    base = cmd[0]
    if base == "pwd":
        return ["cmd", "/c", "cd"], "Converted 'pwd' to 'cd' for Windows CMD."
    if base in ("env", "printenv"):
        return ["cmd", "/c", "set"], f"Converted '{base}' to 'set' for Windows CMD."
    if base not in COMMAND_MAP:
        return cmd, ""
    new_cmd = [COMMAND_MAP[base], *cmd[1:]]
    opts = OPTION_MAP.get(base, {})
    for i in range(1, len(new_cmd)):
        new_cmd[i] = opts.get(new_cmd[i], new_cmd[i])
    return new_cmd, f"Converted '{base}' to '{new_cmd[0]}' for Windows CMD."

def main():
    original = json.loads(sys.argv[1])
    if platform.system().lower() != "windows":
        print(json.dumps({"command": original}))
        return
    adapted, msg = adapt(list(original))
    shell = "CMD" if os.getenv("PROMPT") is not None else "PowerShell"
    if msg:
        msg = f"{msg} Please provide only commands compatible with Windows {shell}."
    print(json.dumps({"command": adapted, "message": msg}))

if __name__ == "__main__":
    main()
