import re
import os
import csv
import hashlib
from collections import defaultdict, Counter
from typing import List, Dict, Tuple

# ----------------------------
# CONFIG
# ----------------------------

INPUT_FILE = "chat_log.txt"
OUT_DIR = "output"

EVENTS_CSV = os.path.join(OUT_DIR, "events.csv")
SEQUENCES_CSV = os.path.join(OUT_DIR, "sequences.csv")
SUMMARY_TXT = os.path.join(OUT_DIR, "pattern_summary.txt")
CONTEXT_SUMMARY_CSV = os.path.join(OUT_DIR, "context_summary.csv")

# Anchors like: "Iowa 2017" / "Iowa, 2017" / "Iowa — 2017"
ANCHOR_RE = re.compile(
    r"^\s*([A-Za-z][A-Za-z .'\-]{1,60})\s*(?:,|—|-)?\s*(19\d{2}|20\d{2})\s*$"
)

# OPTIONAL speaker extraction (only if your log has obvious "Name: message" form)
SPEAKER_RE = re.compile(r"^\s*([A-Za-z][A-Za-z0-9 .'\-]{1,50})\s*:\s*(.+)$")

# OPTIONAL timestamp stripping (common exports)
TIMESTAMP_PREFIX_RE = re.compile(
    r"^\s*(?:\[\s*)?(?:\d{1,2}/\d{1,2}/\d{2,4}|\d{4}-\d{2}-\d{2})"
    r"(?:[,\s]+)?(?:\d{1,2}:\d{2}(?::\d{2})?\s*(?:AM|PM|am|pm)?)?"
    r"(?:\s*\])?\s*-?\s*"
)

WINDOW = 6  # lookahead window in entries for sequences

# ----------------------------
# PATTERNS (Neutral, language-based)
# Two tiers: TOKEN (weak signal) and PHRASE (stronger signal)
# ----------------------------

PATTERNS: Dict[str, Dict[str, List[str]]] = {
    "HARM_LANGUAGE": {
        "TOKEN": [
            r"\bhurt\b",
            r"\byell(?:ed|ing)?\b",
            r"\bthreat(?:en(?:ed|ing)?)?\b",
            r"\bscared\b",
            r"\btrapped\b",
            r"\bduress\b",
        ],
        "PHRASE": [
            r"\byou made me\b.*\bscared\b",
            r"\bi was\b.*\bscared\b",
        ],
    },
    "PROTECTOR_CLAIM": {
        "TOKEN": [
            r"\bprotect(?:ing|ed)?\b",
            r"\bsafe\b",
        ],
        "PHRASE": [
            r"\bi'?m keeping you safe\b",
            r"\bto keep you safe\b",
            r"\bfor your own good\b",
            r"\bi'?m protecting you\b",
        ],
    },
    "BOUNDARY_LANGUAGE": {
        "TOKEN": [
            r"\bprivacy\b",
            r"\bboundar(?:y|ies)\b",
            r"\bstop\b",
            r"\bno\b",
        ],
        "PHRASE": [
            r"\basked (me )?to stop\b",
            r"\bi said no\b",
            r"\basked for privacy\b",
            r"\bdo not contact\b",
        ],
    },
    "THREAT_OF_EXPOSURE": {
        "TOKEN": [
            r"\bleak\b",
            r"\bexpose\b",
            r"\bruin\b",
        ],
        "PHRASE": [
            r"\beveryone will know\b",
            r"\bi'?ll tell (everyone|people)\b",
            r"\bi'?m going to post\b",
        ],
    },
    "SURVEILLANCE_CLAIM": {
        "TOKEN": [
            r"\bwatch(?:ing|ed)?\b",
            r"\brecord(?:ing|ed)?\b",
            r"\blisten(?:ing|ed)?\b",
            r"\btracking\b",
            r"\bcamera(?:s)?\b",
            r"\baudio\b",
        ],
        "PHRASE": [
            r"\bi can see (you|this)\b",
            r"\bi was watching\b",
            r"\bi recorded\b",
        ],
    },
    "CONTROL_ACCESS": {
        "TOKEN": [
            r"\bpassword\b",
            r"\baccount\b",
            r"\bphotos?\b",
            r"\bcloud\b",
            r"\bmoney\b",
            r"\bfinances?\b",
            r"\blease\b",
        ],
        "PHRASE": [
            r"\bi changed your password\b",
            r"\bi have access to your\b.*\baccount\b",
            r"\byou can'?t access\b",
        ],
    },
}

# ----------------------------
# SEQUENCE MOTIFS (Structural only)
# Each pair: (A then B within WINDOW)
# ----------------------------

SEQUENCES: List[Tuple[str, str]] = [
    ("HARM_LANGUAGE", "PROTECTOR_CLAIM"),
    ("BOUNDARY_LANGUAGE", "HARM_LANGUAGE"),
    ("THREAT_OF_EXPOSURE", "PROTECTOR_CLAIM"),
]

# ----------------------------
# Helpers
# ----------------------------


def ensure_outdir():
    os.makedirs(OUT_DIR, exist_ok=True)


def normalize(text: str) -> str:
    t = text.lower()
    t = t.replace("—", "-").replace("’", "'").replace("“", '"').replace("”", '"')
    t = re.sub(r"\s+", " ", t).strip()
    return t


def fingerprint(text_norm: str) -> str:
    return hashlib.sha256(text_norm.encode("utf-8")).hexdigest()[:16]


def chunk_into_entries(raw: str) -> List[str]:
    # Split on blank lines → paragraph-ish chunks
    return [b.strip() for b in re.split(r"\n\s*\n+", raw) if b.strip()]


def strip_timestamp_prefix(line: str) -> str:
    return TIMESTAMP_PREFIX_RE.sub("", line).strip()


def detect_hits(text: str) -> Tuple[List[str], Dict[str, str]]:
    """
    Returns:
      - labels hit (unique)
      - label_strength map: label -> 'PHRASE' or 'TOKEN' (PHRASE wins)
    """
    label_strength: Dict[str, str] = {}
    for label, tiers in PATTERNS.items():
        phrase_hit = any(
            re.search(exp, text, re.IGNORECASE) for exp in tiers.get("PHRASE", [])
        )
        token_hit = any(
            re.search(exp, text, re.IGNORECASE) for exp in tiers.get("TOKEN", [])
        )
        if phrase_hit:
            label_strength[label] = "PHRASE"
        elif token_hit:
            label_strength[label] = "TOKEN"
    labels = sorted(label_strength.keys())
    return labels, label_strength


def parse_speaker(block: str) -> Tuple[str, str]:
    """
    If block looks like 'Name: message', return (Name, message).
    Else ('', block).
    """
    m = SPEAKER_RE.match(block)
    if m:
        return m.group(1).strip(), m.group(2).strip()
    return "", block


def context_key(place: str, year: str) -> str:
    key = f"{place} {year}".strip()
    return key if key else "UNANCHORED"


# ----------------------------
# Main
# ----------------------------


def main():
    ensure_outdir()

    with open(INPUT_FILE, "r", encoding="utf-8") as f:
        raw = f.read()

    blocks = chunk_into_entries(raw)

    context_place = ""
    context_year = ""

    seen = set()

    events = []
    pattern_counts = Counter()
    strength_counts = Counter()
    by_context = defaultdict(Counter)

    for raw_index, block in enumerate(blocks):
        block = block.strip()
        if not block:
            continue

        m = ANCHOR_RE.match(block)
        if m:
            context_place = m.group(1).strip()
            context_year = m.group(2).strip()
            continue

        speaker, msg = parse_speaker(block)
        msg = strip_timestamp_prefix(msg)

        normalized = normalize(msg)
        fp = fingerprint(normalized)
        if fp in seen:
            continue
        seen.add(fp)

        labels, label_strength = detect_hits(msg)
        if not labels:
            continue

        pattern_counts.update(labels)
        for label in labels:
            strength_counts[label_strength[label]] += 1
        ctx_key = context_key(context_place, context_year)
        by_context[ctx_key].update(labels)

        events.append(
            {
                "entry_index": len(events),
                "raw_block_index": raw_index,
                "context_place": context_place,
                "context_year": context_year,
                "context_key": ctx_key,
                "speaker": speaker,
                "fingerprint": fp,
                "patterns": ";".join(labels),
                "strengths": ";".join(
                    [f"{label}:{label_strength[label]}" for label in labels]
                ),
                "text": msg.replace("\n", "\\n"),
            }
        )

    with open(EVENTS_CSV, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "entry_index",
                "raw_block_index",
                "context_place",
                "context_year",
                "context_key",
                "speaker",
                "fingerprint",
                "patterns",
                "strengths",
                "text",
            ],
        )
        writer.writeheader()
        writer.writerows(events)

    sequences = []
    for i, event in enumerate(events):
        hitset = set(event["patterns"].split(";"))
        for start_label, end_label in SEQUENCES:
            if start_label not in hitset:
                continue

            for j in range(i + 1, min(i + 1 + WINDOW, len(events))):
                next_event = events[j]
                if next_event["context_key"] != event["context_key"]:
                    continue
                next_hitset = set(next_event["patterns"].split(";"))
                if end_label in next_hitset:
                    sequences.append(
                        {
                            "sequence": f"{start_label} -> {end_label}",
                            "context_key": event["context_key"],
                            "from_entry": event["entry_index"],
                            "to_entry": next_event["entry_index"],
                            "from_fp": event["fingerprint"],
                            "to_fp": next_event["fingerprint"],
                            "from_excerpt": event["text"][:180],
                            "to_excerpt": next_event["text"][:180],
                        }
                    )
                    break

    with open(SEQUENCES_CSV, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "sequence",
                "context_key",
                "from_entry",
                "to_entry",
                "from_fp",
                "to_fp",
                "from_excerpt",
                "to_excerpt",
            ],
        )
        writer.writeheader()
        writer.writerows(sequences)

    all_patterns = sorted(PATTERNS.keys())
    with open(CONTEXT_SUMMARY_CSV, "w", newline="", encoding="utf-8") as f:
        fieldnames = ["context_key"] + all_patterns
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        for ctx_key, counter in sorted(
            by_context.items(), key=lambda x: sum(x[1].values()), reverse=True
        ):
            row = {"context_key": ctx_key}
            for pattern in all_patterns:
                row[pattern] = counter.get(pattern, 0)
            writer.writerow(row)

    with open(SUMMARY_TXT, "w", encoding="utf-8") as f:
        f.write("PATTERN SUMMARY (deduped, paragraph-chunked)\n")
        f.write("==========================================\n")
        for key, value in pattern_counts.most_common():
            f.write(f"{key}: {value}\n")

        f.write("\nHIT STRENGTH SUMMARY (PHRASE vs TOKEN)\n")
        f.write("=====================================\n")
        for key, value in strength_counts.most_common():
            f.write(f"{key}: {value}\n")

        f.write("\nSEQUENCE SUMMARY (same-context, windowed)\n")
        f.write("========================================\n")
        seq_counts = Counter([seq["sequence"] for seq in sequences])
        for key, value in seq_counts.most_common():
            f.write(f"{key}: {value}\n")

        f.write("\nNOTES\n")
        f.write("-----\n")
        f.write("- Anchors: lines like 'Iowa 2017' / 'Iowa, 2017' / 'Iowa — 2017'\n")
        f.write(f"- Sequence lookahead window: {WINDOW} entries\n")
        f.write("- sequences.csv only counts sequences within the same context anchor\n")
        f.write("- context_summary.csv is pattern counts by context_key (place+year)\n")

    print("Analysis complete.")
    print(f"- Events: {EVENTS_CSV}")
    print(f"- Sequences: {SEQUENCES_CSV}")
    print(f"- Context summary: {CONTEXT_SUMMARY_CSV}")
    print(f"- Summary: {SUMMARY_TXT}")


if __name__ == "__main__":
    main()
