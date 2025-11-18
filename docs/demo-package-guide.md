# PyRouette + StagePort Demo Package

This repository now ships a curated demo package for PyRouette choreography delivery and StagePort credential validation. The bundle includes Docker deployment assets, a credential validator, sample receipts, outreach templates, and a landing page.

## Package contents

The `demo/full_package` directory contains the canonical files that are copied into the generated bundle:

- `PyRouette_CLIENT_COPY_PROTO_2.txt`: Sample PyRouette DSL client copy for Glitch_01.
- `StagePort_Credentialing_Strategy.docx`: Credentialing and minting overview for StageCred.
- `reason_receipt_example.json`: Verifiable credential receipt example.
- `Dockerfile.pyrouette`: Minimal FastAPI validator image.
- `docker-compose.yml`: One-click PyRouette + StagePort validator stack.
- `credential_validator.py`: CLI/HTTP validator for StagePort-style credentials.
- `job_scraping_script.py`: Upwork scraping helper for lead generation.
- `send_email_template.txt`: Outreach email template for PyRouette pitches.
- `landing_page.html`: Simple sales landing page with checkout links.

## Generate the bundle

Use the helper script to copy the curated files to a target directory and compress them into a zip archive.

```bash
python scripts/generate_demo_package.py --output-dir dist/demo_full_package
```

- The copied files live in `dist/demo_full_package`.
- The zip archive is written to `dist/demo_full_package.zip` by default. Pass `--zip-name` to customize the archive filename.

## Running the validator stack

1. Ensure Docker and Docker Compose are installed.
2. From the generated bundle directory, build and start the services:

```bash
docker compose up --build
```

- FastAPI validator: <http://localhost:8000>
- StagePort validator: <http://localhost:8001>

To run the credential validator once without Docker:

```bash
python demo/full_package/credential_validator.py --input demo/full_package/reason_receipt_example.json
```

The script writes reports to `credentials_log.json` and prints the validation response.

## Lead-generation helpers

- The Upwork scraper (`job_scraping_script.py`) fetches recent posts that mention PyRouette. Install `requests` and `beautifulsoup4` before running the script.
- The email template (`send_email_template.txt`) can be personalized for outbound pitches.

These assets ship inside the generated zip so the sales and deployment paths can be shared in a single download.
