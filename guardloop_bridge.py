#!/usr/bin/env python3

import sys
import json
import sqlite3
import os

DB_PATH = os.path.expanduser("~/.guardloop/data.db")

def init_database():
    """
    Initializes the SQLite database and the 'failures' table if they don't exist.
    """
    os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS failures (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            prompt TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )
    """)
    conn.commit()
    conn.close()

def log_failure(prompt):
    """
    Logs a failed prompt to the database.
    """
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("INSERT INTO failures (prompt) VALUES (?)", (prompt,))
    conn.commit()
    conn.close()

def classify_task(prompt):
    """
    Classifies the task based on the prompt.
    """
    code_keywords = ["implement", "function", "class", "debug", "test", "fix"]
    creative_keywords = ["write", "blog post", "email", "poem", "summarize"]

    prompt_lower = prompt.lower()
    if any(keyword in prompt_lower for keyword in code_keywords):
        return {"classification": "code", "confidence": 0.95}
    if any(keyword in prompt_lower for keyword in creative_keywords):
        return {"classification": "creative", "confidence": 0.92}
    return {"classification": "unknown", "confidence": 0.5}

def get_guardrails_for_task(classification):
    """
    Returns guardrails based on the task classification.
    """
    if classification["classification"] == "code":
        return [
            "## Guardrail: Code Standard",
            "- All functions must have a docstring.",
            "- Wrap async database calls in try-catch blocks."
        ]
    return []

def main():
    """
    Main function to handle commands for logging failures or getting guardrails.
    """
    init_database()

    if "--log-failure" in sys.argv:
        try:
            prompt_index = sys.argv.index("--log-failure") + 1
            prompt = sys.argv[prompt_index]
            log_failure(prompt)
            print(json.dumps({"status": "failure logged"}))
        except (ValueError, IndexError):
            print(json.dumps({"error": "No prompt provided for failure logging."}), file=sys.stderr)
            sys.exit(1)
        return

    prompt = sys.argv[1] if len(sys.argv) > 1 else ""
    if not prompt:
        sys.exit(0)

    classification = classify_task(prompt)
    guardrails = get_guardrails_for_task(classification)

    output = {
        "classification": classification,
        "guardrails": "\n".join(guardrails)
    }
    print(json.dumps(output))

if __name__ == "__main__":
    main()