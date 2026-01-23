"""Generate the CLASSIFIED™ one-page historical record PDF."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List

from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import inch
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
from reportlab.platypus import ListFlowable, ListItem, Paragraph, SimpleDocTemplate, Spacer


@dataclass
class BulletSection:
    title: str
    bullets: List[str]


TITLE = "CLASSIFIED\u2122 — ONE-PAGE HISTORICAL RECORD"
SUBTITLE = "Hudson Valley Ballet / Bardavon Lineage (2002–2006)"
METHOD_LINE = "Method: Artifact-based documentation only. No inference. No allegation."

INSTITUTIONAL_SPINE = [
    (
        "Bardavon 1869 Opera House — Poughkeepsie, New York. "
        "Historic regional performing arts venue hosting Ballet Arts Studio and "
        "Dutchess Dance Company productions during the early 2000s."
    ),
    (
        "Ballet Arts Studio — Beacon, New York. Founded 1961 (Elizabeth Schneider Hanson). "
        "Ownership lineage documented as:"
    ),
]

OWNERSHIP_LINEAGE = BulletSection(
    "Ownership lineage",
    [
        "Elizabeth Schneider Hanson (1961–1969)",
        "“Madame Seda” (post-1969)",
        "Valerie Feit (studio director/operator, 1984–2006)",
        "Alex Bloomstein (studio acquisition, 2006)",
    ],
)

CURRICULUM_NOTE = (
    "Curriculum during this period included ballet, modern, African, and jazz dance, "
    "with later expansion into tap, hip hop, and musical theater."
)

PRIMARY_WORK_NODE = BulletSection(
    "Primary Work Node",
    [
        "Wade in the Water",
        "Choreographer/Director: Valerie Feit",
        "Venue: Bardavon Opera House",
        "Documented performance window: 2002–2006 (multi-year span)",
    ],
)

NAMED_ADULTS = BulletSection(
    "Named Adults / Authorities",
    [
        "Valerie Feit — Director/choreographer; Ballet Arts Studio operator (1984–2006).",
        "Maureen Mansfield — Named adult; role pending artifact confirmation.",
        "Yohav Kadar — Named adult; role pending artifact confirmation.",
        "“Miss Dani” — Instructor reference; Bardavon Opera House (2002).",
        "Marge Schiffini — Named adult; parent of Lauren Schiffini.",
        "Alex Bloomstein — Studio acquisition and leadership transition (2006).",
    ],
)

PEERS = BulletSection(
    "Peers / Ensemble Context",
    [
        "Danika Manso-Brown",
        "Abigail (Abby) Glassberg",
        "Alessia Gaetana Santoro",
        "Mariette DelVecchio Scott",
        "Lauren Schiffini",
    ],
)

KEY_EVENT = "2006 — Ownership transfer of Ballet Arts Studio from Valerie Feit to Alex Bloomstein."

EVIDENCE_STATUS = BulletSection(
    "Evidence Status",
    [
        "Confirmed: Bardavon venue; Ballet Arts Studio ownership timeline; Valerie Feit leadership through 2006; Alex Bloomstein acquisition.",
        "Provisional: Annual cast lists and instructional role documentation pending program artifacts.",
    ],
)

USE_LIMITATION = (
    "This record documents institutional presence and chronology only. "
    "It asserts no intent and makes no allegations."
)

DEFAULT_OUTPUT = Path("Classified_Hudson_Valley_Ballet_Bardavon_2002_2006.pdf")


def build_styles() -> dict:
    pdfmetrics.registerFont(UnicodeCIDFont("HeiseiMin-W3"))
    styles = getSampleStyleSheet()
    styles.add(
        ParagraphStyle(
            name="Title",
            parent=styles["Title"],
            fontName="HeiseiMin-W3",
            leading=22,
            spaceAfter=8,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Subtitle",
            parent=styles["Heading2"],
            fontName="HeiseiMin-W3",
            leading=16,
            spaceAfter=10,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Body",
            parent=styles["BodyText"],
            fontName="HeiseiMin-W3",
            leading=15,
            spaceAfter=8,
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
    return styles


def build_bullet_list(items: Iterable[str], styles: dict) -> ListFlowable:
    bullet_items = [ListItem(Paragraph(text, styles["Body"]), leftIndent=10) for text in items]
    return ListFlowable(bullet_items, bulletType="bullet", start="•", leftIndent=14)


def build_story(styles: dict) -> list:
    story: list = []
    story.append(Paragraph(TITLE, styles["Title"]))
    story.append(Paragraph(SUBTITLE, styles["Subtitle"]))
    story.append(Paragraph(METHOD_LINE, styles["Body"]))

    story.append(Paragraph("Institutional Spine", styles["SectionHeader"]))
    for paragraph in INSTITUTIONAL_SPINE:
        story.append(Paragraph(paragraph, styles["Body"]))
    story.append(build_bullet_list(OWNERSHIP_LINEAGE.bullets, styles))
    story.append(Paragraph(CURRICULUM_NOTE, styles["Body"]))

    story.append(Paragraph(PRIMARY_WORK_NODE.title, styles["SectionHeader"]))
    story.append(build_bullet_list(PRIMARY_WORK_NODE.bullets, styles))

    story.append(Paragraph(NAMED_ADULTS.title, styles["SectionHeader"]))
    story.append(build_bullet_list(NAMED_ADULTS.bullets, styles))

    story.append(Paragraph(PEERS.title, styles["SectionHeader"]))
    story.append(build_bullet_list(PEERS.bullets, styles))

    story.append(Paragraph("Key Structural Event", styles["SectionHeader"]))
    story.append(Paragraph(KEY_EVENT, styles["Body"]))

    story.append(Paragraph(EVIDENCE_STATUS.title, styles["SectionHeader"]))
    story.append(build_bullet_list(EVIDENCE_STATUS.bullets, styles))

    story.append(Paragraph("Use Limitation", styles["SectionHeader"]))
    story.append(Paragraph(USE_LIMITATION, styles["Body"]))
    story.append(Spacer(1, 0.1 * inch))
    return story


def create_pdf(output_path: Path) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    styles = build_styles()
    doc = SimpleDocTemplate(
        str(output_path),
        pagesize=letter,
        leftMargin=0.5 * inch,
        rightMargin=0.5 * inch,
        topMargin=0.6 * inch,
        bottomMargin=0.6 * inch,
    )
    doc.build(build_story(styles))
    return output_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate the CLASSIFIED one-page historical record PDF."
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
    print(f"Created CLASSIFIED record at {pdf_path.resolve()}")


if __name__ == "__main__":
    main()
