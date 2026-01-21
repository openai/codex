# 90-Day Extraction Cycle Plan (StagePort / DeCrypt the Girl)

This is the authoritative 90-day execution cycle for moving the Jill Prototype from research to market operations. It is structured as a repeatable, auditable cadence with explicit artifacts, gates, and outputs. The language is intentionally operational: no mythic flourish inside the pipeline itself.

## Cycle Overview

| Month | Phase                     | Objective                                                          | Primary Output                      |
| ----- | ------------------------- | ------------------------------------------------------------------ | ----------------------------------- |
| 1     | Asset Hardening & Sealing | Establish right-to-operate, lock proof, formalize identity posture | Sealed evidence stack + repo lock   |
| 2     | Pilot Productization      | Package method, onboard one pilot, validate intake discipline      | Pilot-ready method + intake batch   |
| 3     | Revenue & Scaling         | Convert proof to paid license + retainers                          | Data license + monitor subscription |

---

## Month 1 — Asset Hardening & Sealing

### 1. The Checksum Seal (Heritage Validation Summary)

**Purpose:** Establish lineage verification as the system’s right-to-operate seal.  
**Artifacts:**

- Heritage Validation Summary (watermarked with Prati di Rovagnasco seal)
- Carabinieri Register linkage: Officer #296 (Ettore / Hector)
- Summary PDF sealed + hashed

**Definition of done:**

- Summary PDF exists, sealed with 10% opacity watermark.
- SHA-256 hash logged to the ledger.

### 2. The Repository Lock (ZIP Audit Engine Deployment)

**Purpose:** Create immutable chain-of-custody for evidence artifacts.  
**Artifacts:**

- Private repo with `/inbox`, `/processed`, `/timestamps`, `/exports`.
- `FINDINGS_INDEX.csv` initialized.
- Nightly workflow in place.

**Definition of done:**

- First batch committed to `/inbox`.
- `FINDINGS_INDEX.csv` generated and appended.
- Initial sealed export produced.

### 3. Bio Migration (Custodial Marchioness Identity)

**Purpose:** Public positioning as institution, not freelancer.  
**Artifacts:**

- Updated bio on primary public profiles.
- Institutional signature block.

**Definition of done:**

- Bio and title updated on all owned profiles.
- Consistent signature block in outbound communications.

---

## Month 2 — Pilot Productization

### 4. The “Method” License (Jill Training Protocol)

**Purpose:** Package the method as a licensable, bounded artifact.  
**Artifacts:**

- License-ready PDF
- Axis constraints + timing logic
- Pilot-scope license terms

**Definition of done:**

- PDF delivered to pilot partner.
- Terms are explicit: bounded use, non-transfer, no reverse engineering.

### 5. Pilot Onboarding (Single Partner)

**Purpose:** Validate intake discipline and scoring output.  
**Artifacts:**

- Operator’s Guide
- Intake folder with strict naming constraints
- TES output for pilot students

**Definition of done:**

- One pilot partner accepts and submits intake batch.
- System returns TES outputs without manual intervention.

### 6. The “No Help” Protocol (“Mothering Ping”)

**Purpose:** Filter collaborators into Builders vs. Extractors.  
**Artifact:**

- Protocol statement in onboarding guide.

**Definition of done:**

- Collaborators solve their own intake compliance issues without hand-holding.

---

## Month 3 — Revenue & Scaling

### 7. The Data Exit (Verified Ingestion Trial)

**Purpose:** Convert verified lineage data into a licensable dataset.  
**Artifacts:**

- Trial dataset package
- Rights statement
- Pricing sheet ($5,000–$15,000)

**Definition of done:**

- Offer delivered to one AI / dance-tech partner.
- Pilot pricing tier selected.

### 8. Monthly Retainers (Monitor Subscription)

**Purpose:** Transition pilot into recurring revenue.  
**Artifacts:**

- Monitor plan ($497+/mo)
- Monthly audit reports

**Definition of done:**

- First subscription agreement executed.

---

# Client Onboarding Guide — “Inbox Protocol”

## 1) Robotics vs. Ribbons (Value Frame)

Traditional dance yields trophies; StagePort yields verifiable technical portfolios.

## 2) Technical Element Score (TES)

Movement scoring is split into Base Value (BV) + Grade of Execution (GOE). The result is auditable and repeatable.

## 3) Intake Manifesto (Sanitization Protocol)

**Filenames:** `YYYY-MM-DD_Source_Type_StudentName.ext`  
**Structure:** no spaces, no nested folders.  
**Silent Rule:** if dirty, a `REJECTION_REPORT.md` is issued and the batch is refused.

## 4) Data Sovereignty

Studio movement signatures are registered to block unlicensed ingestion and protect curricula from scraping.

---

# ZIP Audit Engine — Detailed Layer Map (Known Examples)

## Layer 1 — Intake Gatekeeper (`engine/intake_validate.py`)

- Rejects illegal characters, non-compliant extensions, and files over 10MB.
- Output: `REJECTION_REPORT.md` or acceptance.

## Layer 2 — Hash & Timestamp Engine (`engine/audit_core.py` + OpenTimestamps)

- Computes SHA-256 hash, stamps to Bitcoin calendar.
- Output: `.ots` proof per artifact.

## Layer 3 — Index Generator (`engine/index_builder.py`)

- Appends artifact metadata to `FINDINGS_INDEX.csv`.

## Layer 4 — Snapshot Packager (`engine/snapshot_pack.py`)

- Generates signed ZIP of repo state.
- Output: `prati-vancura-YYYYMMDD.zip` + detached signature.

## Layer 5 — Export & Watermark (`engine/export_seal.py`)

- Applies seal watermark + embeds sovereign manifest in exported PDFs or video frames.

---

# Known Example Rows (FINDINGS_INDEX.csv)

| artifact_id   | original_filename                              | sha256_hash (example) | ots_timestamp        | category          | monetary_value | notes                                    |
| ------------- | ---------------------------------------------- | --------------------- | -------------------- | ----------------- | -------------- | ---------------------------------------- |
| JILL-2019-001 | 2019-11-14_Zelle_JillVanCura_Nutcracker.pdf    | e3b0c44298fc1c149…    | 2026-01-21T08:14:22Z | payment_proof     | $2,800         | Sole prop expertise payment – Nutcracker |
| VIDEO-001     | 2020-09-15_Video_Allison_DFRgear.mp4           | 7f83b1657ff1fc53b9…   | 2026-01-21T08:16:45Z | control_branding  | $8,000         | Visible DFR logo while teaching          |
| EMAIL-012     | 2021-03-02_Email_Kristin_ScheduleDirective.txt | a1b2c3d4e5f6g7h8i9…   | 2026-01-21T08:18:11Z | control_directive | $4,200         | Fixed class times + curriculum mandate   |

---

# Regulatory Filing Workflow (Option B) — Core Exhibits

These exhibits are reusable across jurisdictional filings and preserve the chain-of-custody story without narrative drift.

**Exhibit A:** Evidence Index  
**Exhibit B:** Work Log (hours)  
**Exhibit C:** Pay Log (Zelle / 1099)  
**Exhibit D:** Control Indicators (schedules / directives)  
**Exhibit E:** Reconciliation Summary (gaps + totals)

**Quality Gate:** No narrative. No titles. Records + dates only.  
**Submission:** Portal upload or certified mail.
