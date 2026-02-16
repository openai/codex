from __future__ import annotations

import json
from pathlib import Path


def test_notebooks_are_valid_and_have_code_cells():
    notebooks_dir = Path(__file__).resolve().parents[1] / "notebooks"
    files = sorted(notebooks_dir.glob("*.ipynb"))
    assert files, "No notebooks found"

    for nb_path in files:
        data = json.loads(nb_path.read_text())
        assert data.get("nbformat") == 4
        cells = data.get("cells", [])
        assert cells, f"Notebook has no cells: {nb_path.name}"
        assert any(c.get("cell_type") == "code" for c in cells), f"Notebook has no code cells: {nb_path.name}"
