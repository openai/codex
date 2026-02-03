# Advocacy Automation (Justice-first, low-leakage)

This folder provides a **local-first** workflow to organize evidence, build advocacy packets, and prepare Gmail/Drive exports **without leaking sensitive data**. Everything runs offline by default. Network use is optional and gated by your explicit action.

## Principles

- **Local-first**: files stay on disk until you explicitly export them.
- **Minimum exposure**: redact before sharing, hash evidence for tamper checks.
- **Draft-only**: prepare `.eml` drafts instead of sending emails directly.
- **Separation of concerns**: raw evidence stays in `data/`, redacted copies go in `packet/`.

## Quick Start

1. Fill out the templates in `templates/`.
2. Place evidence files in `data/evidence/`.
3. Generate hashes for evidence:
   ```bash
   python advocacy_automation/scripts/hash_evidence.py data/evidence
   ```
4. Redact sensitive fields from any document you plan to share:
   ```bash
   python advocacy_automation/scripts/redact.py templates/chronology.csv packet/chronology.redacted.csv
   ```
5. Build a consolidated advocacy packet:
   ```bash
   python advocacy_automation/scripts/build_packet.py
   ```
6. Create Gmail draft files (`.eml`) locally for manual review:
   ```bash
   python advocacy_automation/scripts/gmail_drafts.py
   ```

## Gmail/Drive (No Leakage Mode)

- **Gmail**: The system creates `.eml` draft files in `packet/drafts/`. You can open and send them manually in Gmail, keeping full control.
- **Drive**: A local `drive_manifest.json` is generated to mirror how you want folders organized. You can upload manually (drag/drop) or use your own Drive tools.

## Data Safety Checklist

- [ ] Redact before sharing.
- [ ] Hash evidence files and store hashes with timestamps.
- [ ] Keep raw evidence offline; share only packet outputs.
- [ ] Review every draft before sending.

## Folder Layout

```
advocacy_automation/
├── data/
│   ├── evidence/              # raw files (kept local)
│   └── metadata/              # generated hashes and manifests
├── templates/                 # editable templates
├── packet/                    # redacted + export-ready materials
└── scripts/                   # local-only automation tools
```

## Legal & Safety Note

This toolkit helps **organize and present** information. It is **not** legal advice. If immediate safety is at risk, contact local emergency services.
