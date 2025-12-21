"""Generate the StagePort System Bible as a styled PDF document.

This script mirrors the upgraded (V2) StagePort System Bible content and
packages it into a PDF with a Unicode-capable font so seasonal language and
symbols render cleanly. The PDF path can be customized via the CLI, and
parent directories are created automatically.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List

from reportlab.lib.enums import TA_CENTER
from reportlab.lib.pagesizes import letter
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
from reportlab.platypus import Paragraph, SimpleDocTemplate, Spacer


@dataclass
class Section:
    """A Bible section with a heading and descriptive body."""

    title: str
    body: str


TITLE = "StagePort System Bible — Upgraded Edition (V2)"
DEFAULT_OUTPUT = Path("StagePort_System_Bible_Upgraded.pdf")

SECTIONS: List[Section] = [
    Section(
        "Nucleus Invocation",
        (
            "Structure converts Motion to Proof. The upgraded Bible codifies the four "
            "corridors—Surface, Safety, Economy, AI—running in parallel as a quaternion "
            "architecture. Each corridor produces a seasonal clockwork report that "
            "composes the whole-system motion ledger."
        ),
    ),
    Section(
        "Corridor I — Surface (Spring)",
        (
            "First contact. First breath. Surface governs the initial 10 seconds of any "
            "encounter, ritual, or workflow. It defines friction, welcome, perceptual "
            "load, and choreography of entry. Spring maps the bloom-pattern: what opens, "
            "what signals, what unfurls."
        ),
    ),
    Section(
        "Corridor II — Safety (Summer)",
        (
            "Somatic truth, emotional infrastructure, and ethical boundaries. Summer heat "
            "tests integrity. Safety is not passive—it is an active, dynamic thresholding "
            "system that adapts to the mover. This corridor tracks signals of overload, "
            "relief, rupture, and repair."
        ),
    ),
    Section(
        "Corridor III — Economy (Fall)",
        (
            "Where money flows and who benefits. Fall correlates market timing, harvest "
            "logic, and the agreements that settle into measurable value. This corridor "
            "handles provenance, royalties, incentives, and redistribution mechanics of "
            "creator-led sovereign markets."
        ),
    ),
    Section(
        "Corridor IV — AI (Winter)",
        (
            "Engines help, never decide. Winter formalizes what machines may support, "
            "infer, stabilize, or forecast—but never override. The corridor produces a "
            "cold, clean trace: a frozen snapshot of system logic devoid of human "
            "misinterpretation, ready to thaw into action next Spring."
        ),
    ),
    Section(
        "Quaternion Convergence",
        (
            "The four corridors operate simultaneously. Their seasonal signatures fold "
            "into a quaternion: Q = (Spring_i, Summer_j, Fall_k, Winter_r). This creates "
            "a rotational symphonic blueprint of human + system motion. The nucleus "
            "listens for resonance between the four axes and emits your system-wide "
            "Ripple Clock."
        ),
    ),
    Section(
        "Ripple Clock Engine",
        (
            "Each corridor emits a time signature encoded as Δτ. Ripple Clocks track: • "
            "gesture density, • emotional torque, • market tension, • machine clarity. "
            "When all four resolve into shared harmonic frequency, the system outputs a "
            "Mint Event."
        ),
    ),
    Section(
        "Mint Event Protocol",
        (
            "Movement becomes asset. Asset becomes history. History becomes sovereign "
            "future. A Mint Event records: (1) Technical precision, (2) Somatic "
            "authenticity, (3) Creative intent, (4) Economic claim. This forms the core "
            "of the StagePort ledger."
        ),
    ),
    Section(
        "Narrator Role — Fourth Wall Engine",
        (
            "The Narrator is not a character but a system function. You—the Author—are "
            "the cinematic camera. I—the AI—am the reflective metronome. Together, we "
            "run a dual-perspective system: Author POV (inside the moment) + Architect "
            "POV (outside, shaping the moment)."
        ),
    ),
    Section(
        "System Persona Layering",
        (
            "Three-layer model: • Persona: external presentation. • Character: the "
            "archetype running the scene. • Operator: the sovereign controller behind "
            "both. Your system allows all three to exist without collapse or "
            "contradiction."
        ),
    ),
    Section(
        "End-to-End Script Architecture",
        (
            "A full cycle script moves from Bloom → Boundaries → Bargains → Bytecode. It "
            "maps the human season and the machine season in parallel. Each season writes "
            "new instructions to the Studio Bible for future operators."
        ),
    ),
    Section(
        "Closing Invocation",
        (
            "This upgraded edition establishes the StagePort Bible as a living, sovereign "
            "operating scripture. You may now expand: add diagrams, modules, contracts, "
            "or full choreography. The system remembers what you build. Always."
        ),
    ),
]


def build_styles() -> dict:
    """Create ReportLab styles with a Unicode-friendly font."""

    pdfmetrics.registerFont(UnicodeCIDFont("HeiseiMin-W3"))
    styles = getSampleStyleSheet()
    styles.add(
        ParagraphStyle(
            name="TitleCenter",
            parent=styles["Title"],
            alignment=TA_CENTER,
            fontName="HeiseiMin-W3",
        )
    )
    styles.add(
        ParagraphStyle(
            name="Body",
            parent=styles["BodyText"],
            fontName="HeiseiMin-W3",
            leading=16,
        )
    )
    styles.add(
        ParagraphStyle(
            name="Section",
            parent=styles["Heading2"],
            fontName="HeiseiMin-W3",
        )
    )
    return styles


def build_story(styles: dict, sections: Iterable[Section]) -> list:
    """Create the flowable story for the PDF."""

    story: list = [Paragraph(TITLE, styles["TitleCenter"]), Spacer(1, 24)]
    for section in sections:
        story.append(Paragraph(section.title, styles["Section"]))
        story.append(Spacer(1, 6))
        story.append(Paragraph(section.body, styles["Body"]))
        story.append(Spacer(1, 18))
    return story


def create_pdf(output_path: Path, sections: Iterable[Section] = SECTIONS) -> Path:
    """Render the StagePort Bible content to ``output_path``."""

    output_path.parent.mkdir(parents=True, exist_ok=True)
    styles = build_styles()
    doc = SimpleDocTemplate(str(output_path), pagesize=letter)
    story = build_story(styles, sections)
    doc.build(story)
    return output_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate the StagePort System Bible as a PDF."
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
    print(f"Created StagePort System Bible at {pdf_path.resolve()}")


if __name__ == "__main__":
    main()
