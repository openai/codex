# Local, Offline Evidence Kit (Free + Secure)

This guide keeps everything on your device—no cloud portals, no paid tools, and no internet required after initial installs. Use it to build and maintain a local evidence timeline that stays under your control.

## Quick Setup (Local Only)

1. **Install Python (if needed).** Use the official Python installer for your OS. After install, you can run scripts locally without any internet connection.
2. **Use a text editor.** Any basic editor works (Notepad on Windows, TextEdit on macOS, or a code editor if you prefer).
3. **Run scripts locally.** Open a terminal, navigate to the folder, and run: `python build_evidence.py`.
4. **Why local?** Everything stays on your machine. Back up to an external drive or encrypted folder.

## Starter Data + Build Script

Save the script below as `build_evidence.py` in a new folder. It generates three local files:

- `evidence_timeline.csv` (editable in a spreadsheet app)
- `evidence_timeline.json` (portable backup)
- `evidence_dashboard.html` (local, offline dashboard you can open in any browser)

```python
import pandas as pd

# Your data here - edit/add rows as needed
DATA = {
    "Date": [
        "2019-09-17",
        "2019-09-25",
        "2020-03-01",
        "2020-12-01",
        "2021-10-01",
        "2021-11-01",
        "2021-12-01",
        "2022-01-01",
        "2022-02-01",
        "2022-03-01",
        "2025-12-12",
        "2025-12-13",
    ],
    "Event": [
        "Role start via Indeed, social media access email",
        "Zelle payment $637 from Dance Factory Ridgefield, LLC",
        "COVID pivot to digital/video productions",
        "Nutcracker production at Ridgefield elementary school",
        "Nutcracker 2021 music editing starts",
        "Ongoing rehearsals and work",
        "Nutcracker 2021 performance and live stream",
        "Departure from studio",
        "2021 1099-NEC $17,841 mailed",
        "Storage unit delinquency notice, property disposal",
        "Nutcracker Dress Rehearsal",
        "Nutcracker Performance and Winter Showcase",
    ],
    "Payment": [
        "N/A",
        "$637 (Zelle)",
        "No payment recorded (possible unpaid period)",
        "No payment recorded",
        "No payment recorded",
        "No payment recorded",
        "No payment recorded",
        "N/A",
        "$17,841 (1099 for 2021)",
        "N/A",
        "N/A",
        "N/A",
    ],
    "Notes": [
        "Operational integration begins; immediate assignments without upfront pay agreement.",
        "First known payment after initial work.",
        "Shift to remote work; potential PPP overlap.",
        "Video-based show over COVID.",
        "Direct evidence of production-level authorship (music folder).",
        "Continued prep for Nutcracker.",
        "Livestream ticket sales using created assets.",
        "Access cut-off begins.",
        "Tax form for previous year.",
        "Mail and property control.",
        "Current year rehearsal (post-your involvement).",
        "Current year performance.",
    ],
}

# Create DataFrame (single sheet structure)
df = pd.DataFrame(DATA)

# Export to CSV (editable in a spreadsheet app)
df.to_csv("evidence_timeline.csv", index=False)

# Export to JSON (for backups or apps)
df.to_json("evidence_timeline.json", orient="records")

# Generate simple HTML dashboard (local, offline view)
rows = []
for _, row in df.iterrows():
    rows.append(
        """
        <tr>
            <td>{Date}</td>
            <td>{Event}</td>
            <td>{Payment}</td>
            <td>{Notes}</td>
        </tr>
        """.format(**row)
    )

html = """<!DOCTYPE html>
<html lang=\"en\">
<head>
  <meta charset=\"UTF-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />
  <title>Evidence Timeline</title>
  <style>
    body { font-family: Arial, sans-serif; margin: 24px; }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
    th { background: #f4f4f4; }
    caption { text-align: left; font-weight: bold; margin-bottom: 8px; }
  </style>
</head>
<body>
  <table>
    <caption>Evidence Timeline</caption>
    <thead>
      <tr>
        <th>Date</th>
        <th>Event</th>
        <th>Payment</th>
        <th>Notes</th>
      </tr>
    </thead>
    <tbody>
      {rows}
    </tbody>
  </table>
</body>
</html>
""".format(rows="\n".join(rows))

with open("evidence_dashboard.html", "w", encoding="utf-8") as file:
    file.write(html)

print("Files updated: evidence_timeline.csv, evidence_timeline.json, evidence_dashboard.html")
```

## How to Use & Expand

- **Run the script:** `python build_evidence.py` generates/updates files.
- **Add more data:** Edit the `DATA` dictionary (add rows or new columns like `Exhibit ID`). Run again.
- **Open the dashboard:** Double-click `evidence_dashboard.html` to view it locally.
- **Backup safely:** Copy files to a USB drive or an encrypted folder.
- **No Python?** Use a spreadsheet app (like LibreOffice Calc) to edit `evidence_timeline.csv` directly.

## Optional Additions (Tell Me What You Want)

- Insurance timeline section
- Legal timeline section
- Exhibit register with file hashes
- Email and message index
- Expense log and reimbursement tracker

When you’re ready, say what additional sections to add, and I’ll extend the script and template.
