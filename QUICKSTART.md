# LMU Curriculum Runtime - Quick Start

## Prerequisites

Python 3.11+, Ollama installed

## Install Ollama

```bash
# macOS/Linux
curl -fsSL https://ollama.com/install.sh | sh

# Start server
ollama serve

# Pull model (in new terminal)
ollama pull llama2:latest
```

## Setup

```bash
# Install dependencies
pip3 install pyyaml requests

# Test components
python3 test_units.py
```

## Run

```bash
# Verify Ollama
python3 run.py --check

# Generate single lesson
python3 run.py --lessons 0.1

# Generate phase
python3 run.py --phase foundations

# Generate all
python3 run.py

# Check receipts
cat celaya/lmu/artifacts/receipts.jsonl | python3 -m json.tool

# Check summary
cat celaya/lmu/artifacts/generation_summary.json | python3 -m json.tool
```

## Structure

```
celaya/lmu/
├── syllabus/          6 lessons, 4 phases
├── generator/         Prompts, extract, validate
├── runtime/           Receipts, runner (Ollama)
├── grading/           Weights, scorer
└── artifacts/         Output (gitignored)
```

## Troubleshooting

**Ollama not found**
```bash
curl http://localhost:11434/api/tags
```

**Import errors**
```bash
export PYTHONPATH=/home/user/codex:$PYTHONPATH
```

**Test extract manually**
```python
from celaya.lmu.generator.extract import extract_json_from_text
result, err = extract_json_from_text('{"test": true}')
print(result)
```
