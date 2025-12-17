"""Generate the Founding Faculty Premiere Invitation as a print-ready PDF.

This script typesets the one-page invitation for AVC Systems Studio / StagePort.
It mirrors the provided content, centers the header lines, and formats the
body copy with generous leading for a clean, print-friendly layout.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List

from reportlab.lib.enums import TA_CENTER
from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import inch
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
from reportlab.platypus import ListFlowable, ListItem, Paragraph, SimpleDocTemplate, Spacer


@dataclass
class BulletSection:
    """Structured content with a heading and optional bullet list."""

    title: str
    bullets: List[str]


TITLE = "Founding Faculty Premiere Invitation"
SUBTITLE = "AVC Systems Studio • StagePort | DeCrypt the Girl"
DOC_ID = "Doc ID: AVC-FF-ON-0001 • Confidential"

LEAD_LINE = (
    "You are receiving a premiere artifact from AVC Systems Studio — a founder prism "
    "staged as proof. Break the seal to choose your corridor: Teach, Invest, or Build."
)

FOUNDER_STATEMENT = (
    "Allison Van Cura converts 35 years of Balanchine-grade embodied intelligence into "
    "sovereign infrastructure. StagePort and DeCrypt the Girl render movement as data, "
    "timing as protocol, and aesthetics as executable governance. We mint auditable "
    "StageCreds, enforce somatic safety, and route economic rails that compensate "
    "backstage labor."
)

PACKET_CONTENTS = BulletSection(
    "What this packet contains",
    [
        "Pilot Performance Bundle: Stability Index time series; latency and throughput logs; sample StageCred PDF + JSON receipt.",
        "Technical Stack: DeepScorer spec; Director’s Chair memnode; StagePort node schema and API contracts.",
        "Safety & Governance: Title IX intake workflow; anonymized safety logs; MythOS Constitution excerpt.",
        "Economics: Token mint ledger snapshots; Sentient Cents flow; scholarship routing examples.",
        "Operational Artifacts: Pilot roster; curriculum map; Kits lookbook; Artifact Sandbox receipts.",
    ],
)

COVENANT = (
    "We agree to teach with constraint, govern with clarity, and protect embodied labor. "
    "By signing below you accept stewardship of lineage, safety, and timing."
)

CORRIDOR_ACTIONS = BulletSection(
    "Corridor actions",
    [
        "Teach: Curriculum mapping, Faculty Study onboarding, pilot schedule.",
        "Invest: Diligence corridor, term-sheet intent, governance seat options.",
        "Build: Atelier access, prototype demo, Ghost Labor onboarding.",
    ],
)

CONTACT_LINE = "Contact atelier@avc.studio — Director’s Chair access via enclosed QR."
FOOTER = "Private ceremonial invitation — not for public distribution."

DEFAULT_OUTPUT = Path("Founding_Faculty_Premiere_Invitation.pdf")


def build_styles() -> dict:
    """Create ReportLab styles tailored for the one-page invitation."""

    pdfmetrics.registerFont(UnicodeCIDFont("HeiseiMin-W3"))
    styles = getSampleStyleSheet()
    styles.add(
        ParagraphStyle(
            name="TitleCenter",
            parent=styles["Title"],
            alignment=TA_CENTER,
            fontName="HeiseiMin-W3",
            leading=24,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Subtitle",
            parent=styles["Heading2"],
            alignment=TA_CENTER,
            fontName="HeiseiMin-W3",
            textColor="#444444",
            leading=18,
            spaceAfter=4,
        )
    )
    styles.add(
        ParagraphStyle(
            name="DocId",
            parent=styles["BodyText"],
            alignment=TA_CENTER,
            fontName="HeiseiMin-W3",
            leading=14,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Body",
            parent=styles["BodyText"],
            fontName="HeiseiMin-W3",
            leading=17,
            spaceAfter=10,
        )
    )
    styles.add(
        ParagraphStyle(
            name="SectionHeader",
            parent=styles["Heading3"],
            fontName="HeiseiMin-W3",
            spaceBefore=6,
            spaceAfter=4,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Footer",
            parent=styles["BodyText"],
            alignment=TA_CENTER,
            fontName="HeiseiMin-W3",
            leading=14,
            textColor="#555555",
        )
    )
    return styles


def build_bullet_list(items: Iterable[str], styles: dict) -> ListFlowable:
    bullet_items = [ListItem(Paragraph(text, styles["Body"]), leftIndent=10) for text in items]
    return ListFlowable(bullet_items, bulletType="bullet", start="•", leftIndent=14)


def build_story(styles: dict) -> list:
    story: list = []

    story.append(Spacer(1, 0.4 * inch))
    story.append(Paragraph(TITLE, styles["TitleCenter"]))
    story.append(Paragraph(SUBTITLE, styles["Subtitle"]))
    story.append(Paragraph(DOC_ID, styles["DocId"]))
    story.append(Spacer(1, 0.2 * inch))

    story.append(Paragraph("Lead line", styles["SectionHeader"]))
    story.append(Paragraph(LEAD_LINE, styles["Body"]))

    story.append(Paragraph("Founder statement", styles["SectionHeader"]))
    story.append(Paragraph(FOUNDER_STATEMENT, styles["Body"]))

    story.append(Paragraph(PACKET_CONTENTS.title, styles["SectionHeader"]))
    story.append(build_bullet_list(PACKET_CONTENTS.bullets, styles))
    story.append(Spacer(1, 0.1 * inch))

    story.append(Paragraph("Founding Faculty Covenant", styles["SectionHeader"]))
    story.append(Paragraph(COVENANT, styles["Body"]))
    story.append(Paragraph("Signature ____________________", styles["Body"]))
    story.append(Paragraph("Name / Role / Institution ____________________", styles["Body"]))
    story.append(Paragraph("Date ____________________", styles["Body"]))

    story.append(Paragraph(CORRIDOR_ACTIONS.title, styles["SectionHeader"]))
    story.append(build_bullet_list(CORRIDOR_ACTIONS.bullets, styles))
    story.append(Spacer(1, 0.1 * inch))

    story.append(Paragraph("Contact", styles["SectionHeader"]))
    story.append(Paragraph(CONTACT_LINE, styles["Body"]))

    story.append(Spacer(1, 0.2 * inch))
    story.append(Paragraph(FOOTER, styles["Footer"]))

    return story


def create_pdf(output_path: Path) -> Path:
    """Render the invitation to the requested path."""

    output_path.parent.mkdir(parents=True, exist_ok=True)
    styles = build_styles()
    doc = SimpleDocTemplate(
        str(output_path),
        pagesize=letter,
        leftMargin=0.9 * inch,
        rightMargin=0.9 * inch,
        topMargin=0.8 * inch,
        bottomMargin=0.8 * inch,
    )
    story = build_story(styles)
    doc.build(story)
    return output_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate the Founding Faculty Premiere Invitation as a PDF."
    )
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="Destination path for the generated PDF (directories will be created).",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    pdf_path = create_pdf(args.output)
    print(f"Created Founding Faculty Premiere Invitation at {pdf_path.resolve()}")


if __name__ == "__main__":
    main()
