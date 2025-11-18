# Stageport Evolution Quickstart

This guide captures the iPad-friendly workflow for live curriculum evaluation
with `watcher.py` and `stageport_evolution.py`. It mirrors the requested
PyLingo IDE loop while keeping everything runnable from Juno, PyDroid, or a
lightweight code-server session.

## Prerequisites

- Python 3.9+
- `watchdog` for filesystem notifications
- (Optional) `firebase-admin` for MASTER-level logging

Install the dependencies into your mobile or remote environment:

```bash
pip install watchdog firebase-admin
```

## File layout

Save your working files in the same directory:

- `your_curriculum.txt` — the lesson you are editing
- `stageport_evolution.py` — evaluation logic
- `watcher.py` — monitors file changes and prints scores

You can copy the reference implementations from this repository:

- `scripts/stageport/stageport_evolution.py`
- `scripts/stageport/watcher.py`

## Live watcher loop (Juno, PyDroid, or code-server)

1. Place your lesson text in `your_curriculum.txt`.
2. Keep `stageport_evolution.py` and `watcher.py` in the same folder.
3. Run the watcher:

   ```bash
   python3 watcher.py
   ```

4. Each time you save the curriculum file, the watcher will print an updated
   evaluation report. If you want to write a local ledger or log MASTER results
   to Firebase, set the environment variables:

   ```bash
   STAGEPORT_TARGET=your_curriculum.txt \
   STAGEPORT_LEDGER=metadata/ledger.jsonl \
   STAGEPORT_LOG_MASTER=1 \
   STAGEPORT_FIREBASE_CREDENTIALS=/path/to/firebase-service-account.json \
   STAGEPORT_USER_ID=dev-user \
   python3 watcher.py
   ```

A mermaid sketch of the loop:

```mermaid
graph TD
  A[PyLingo IDE] -->|writes lesson| B(watcher.py)
  B -->|validate()| C(stageport_evolution.py)
  C -->|score| D[Terminal Display]
  C -->|if MASTER| E[Firestore: evolution_logs]
  E -->|retrieval| F[GEN AI Assistant]
  F -->|suggest| G[New Level for PyLingo]
  A -->|runs| A
```

## Firebase MASTER logging

When `STAGEPORT_LOG_MASTER=1` is set and `firebase_admin` is installed, the
`validate` function will call `commit_to_evolution` for MASTER-level results.
Provide a service account JSON file via `STAGEPORT_FIREBASE_CREDENTIALS` to
publish records to the `evolution_logs` collection with the following shape:

```json
{
  "score": 97.5,
  "dimensions": {
    "clarity": 98.0,
    "coverage": 96.0,
    "safety": 100.0,
    "flow": 96.0
  },
  "timestamp": 1717181718.1,
  "uid": "dev-user"
}
```

A minimal helper for manual logging is also available in
`scripts/stageport/stageport_evolution.py`:

```python
from stageport_evolution import commit_to_evolution
commit_to_evolution(report, user_id="dev-user", credentials_path="/path/to/firebase-service-account.json")
```

## Running evaluations manually

You can invoke the evaluator directly if you prefer a one-off check:

```bash
python3 stageport_evolution.py your_curriculum.txt \
  --ledger metadata/ledger.jsonl \
  --log-master \
  --firebase-credentials /path/to/firebase-service-account.json \
  --user-id dev-user
```

## Service examples

Systemd unit template for continuous training runs:

```ini
[Unit]
Description=Run stageport evolution trainer
After=network.target

[Service]
ExecStart=/usr/bin/python3 /home/user/project/train_stageport.py
Restart=on-failure
WorkingDirectory=/home/user/project
StandardOutput=append:/var/log/trainer.log
StandardError=append:/var/log/trainer.err
```

Early stopping pattern for model training:

```python
if val_loss < best_loss - delta:
    best_loss = val_loss
    patience_counter = 0
else:
    patience_counter += 1
    if patience_counter >= patience:
        break
```

Utility for writing a JSONL ledger:

```python
def write_ledger(metadata: dict, ledger_path="metadata/ledger.jsonl"):
    with open(ledger_path, "a") as f:
        f.write(json.dumps(metadata) + "\n")
```

These snippets mirror the original request so that learners can set up the
pipeline quickly on constrained devices.
