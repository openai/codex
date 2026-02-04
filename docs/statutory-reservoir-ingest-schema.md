# Statutory Reservoir Ingest Schema (SRI)

**Purpose**
This schema translates intake material into a forensic, statute-labeled index. It replaces generic project categories with statutory anchors so that each intake item is immediately classified as an evidentiary fragment aligned to a legal element. The output is a structured, auditable reservoir entry that can be exported into claims, filings, or reviews without re-interpretation.

**Guardrails**

- This is an indexing and evidence-categorization framework, not legal advice.
- Use precise, factual language. Avoid speculation.
- Every entry must map to a specific artifact or source.

---

## 1) Statutory Code Table (Core Index)

| Statutory Code     | Crime / Violation        | Forensic Element (Evidence Target)                                             |
| ------------------ | ------------------------ | ------------------------------------------------------------------------------ |
| NY PL 135.35       | Labor Trafficking        | Compelling professional services through fraud or induced dependency.          |
| NY PL 155.05(2)(d) | Larceny by False Promise | Inducing liquidation of assets through a false promise (e.g., role or equity). |
| NY PL 155.05(2)(f) | Larceny by Wage Theft    | Failure to pay promised wages or compensation.                                 |
| NY PL 190.65       | Scheme to Defraud        | Systematic course of conduct to obtain property via misrepresentation.         |
| 18 U.S.C. § 1702   | Mail Tampering           | Interference with mail used to monitor or control financial timing.            |
| CT GS 53a-129      | Identity Theft           | Unauthorized use of SSN or identity to shift tax liability.                    |
| 26 U.S.C. § 7206   | Tax Perjury              | Fraudulent tax reporting (e.g., false 1099 to claim a deduction).              |

---

## 2) Forensic Role Translation (Operational Mapping)

| Original Role          | Forensic Role               | Function                                                                                |
| ---------------------- | --------------------------- | --------------------------------------------------------------------------------------- |
| Information Manager    | Statutory Archivist         | Ensures every artifact has timestamp + provenance and is mapped to a statutory element. |
| Specs Lead             | Statutory Mapping Lead      | Maintains “elements of the crime” alignment for new inputs.                             |
| PMO / Project Controls | Audit Cadence Lead          | Runs monthly audits (e.g., circular funding checks).                                    |
| QA/QC                  | Zero-Loss Integrity Monitor | Verifies no labor or asset loss goes unlogged.                                          |
| Resident Engineer      | Chief Forensic Investigator | Issues a single, authoritative ruling on conflicts.                                     |
| BIM / Data Steward     | Financial Forensics Steward | Bridges bank/ledger data into asset IDs and statutory tags.                             |
| Legal                  | Prosecution Liaison         | Formats definitions and evidence for filings.                                           |

---

## 3) Daily Intake Object (Canonical)

```json
{
  "date": "YYYY-MM-DD",
  "entry_type": "STATUTORY_INK",
  "statute": "NY PL 135.35 (Labor Trafficking)",
  "element": "Induced Dependency / Unpaid Professional Services",
  "signal": "Concise, factual description of observed event.",
  "cost": { "asset_liquidated": 0, "hours_extracted": 0 },
  "proof": {
    "artifact_id": "EVID-0000",
    "type": "Receipt / Email / Screenshot"
  },
  "status": "C1 Provable"
}
```

**Status Codes**

- **C0** — Not yet verified (needs artifact)
- **C1** — Provable (artifact in hand)
- **C2** — Corroborated (multiple independent artifacts)

---

## 4) Evidence Integrity Checklist

- **Timestamped artifact**: Screenshot, PDF, receipt, email header, or bank entry.
- **Provenance link**: Source location and collection method.
- **Statutory link**: Which statute and element the artifact supports.
- **Asset ID**: Unique ID for each transaction or item.

---

## 5) Example Entry (Filled)

```json
{
  "date": "2026-02-03",
  "entry_type": "STATUTORY_INK",
  "statute": "NY PL 135.35 (Labor Trafficking)",
  "element": "Induced Dependency / Unpaid Professional Services",
  "signal": "Request for professional deliverables without compensation after repeated dependency pressure.",
  "cost": { "asset_liquidated": 0, "hours_extracted": 15 },
  "proof": { "artifact_id": "EVID-0045", "type": "Email + Project Files" },
  "status": "C1 Provable"
}
```

---

## 6) Reservoir Export (One Row Rule)

Each audit cycle produces a single reservoir row (append-only). If it doesn’t fit in one line, it stays in the aquifer (working notes).

**Reservoir Entry (Template)**

```
[YYYY-MM-DD] — [Statutory Code(s)] — [Primary evidentiary chain in one sentence] — [Status]
```

---

## 7) Implementation Notes

- Use the same statute list across all intake channels.
- Keep the “signal” field factual and short; evidence lives in the artifact.
- Every intake item must have a **statute + element + artifact** or it stays in draft.

This schema is the spine. Once populated, it becomes a portable, auditable evidence reservoir.
