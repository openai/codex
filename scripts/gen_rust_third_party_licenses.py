#!/usr/bin/env python3
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CARGO_LOCK = ROOT / "codex-rs" / "Cargo.lock"


def parse_cargo_lock(lock_path: Path):
    # Minimal TOML parser for packages: name/version/source
    pkgs = []
    if not lock_path.exists():
        return pkgs
    current = {}
    in_pkg = False
    for line in lock_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line == "[[package]]":
            if in_pkg and current.get("name") and current.get("version"):
                pkgs.append(current)
            current = {}
            in_pkg = True
            continue
        if not in_pkg:
            continue
        if line.startswith("name = "):
            current["name"] = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("version = "):
            current["version"] = line.split("=", 1)[1].strip().strip('"')
        elif line.startswith("source = "):
            current["source"] = line.split("=", 1)[1].strip().strip('"')
    if in_pkg and current.get("name") and current.get("version"):
        pkgs.append(current)
    # Filter out workspace/local crates (no source) and own crates
    out = []
    seen = set()
    for p in pkgs:
        key = (p["name"], p["version"])
        if key in seen:
            continue
        seen.add(key)
        src = p.get("source", "")
        if not src:
            continue
        if p["name"].startswith("codex-"):
            continue
        out.append(p)
    return out


def find_crate_dir(cargo_home: Path, name: str, version: str):
    # registry path: ~/.cargo/registry/src/*/<name>-<version>
    src_root = cargo_home / "registry" / "src"
    if src_root.exists():
        for sub in src_root.iterdir():
            cdir = sub / f"{name}-{version}"
            if cdir.exists():
                return cdir
    # git checkouts are trickier; best-effort scan
    git_root = cargo_home / "git" / "checkouts"
    if git_root.exists():
        pattern = re.compile(re.escape(name), re.IGNORECASE)
        for sub in git_root.iterdir():
            if not pattern.search(sub.name):
                continue
            # try subdirs
            for child in sub.iterdir():
                if child.is_dir():
                    return child
    return None


LICENSE_RE = re.compile(r"^(license|licence|copying|copyright|unlicense)(\.|$)", re.IGNORECASE)


def pick_license_files(crate_dir: Path):
    files = []
    try:
        for p in crate_dir.iterdir():
            if p.is_file() and LICENSE_RE.match(p.name):
                files.append(p)
    except Exception:
        return []
    # Prefer LICENSE* first
    files.sort(key=lambda p: (0 if p.name.lower().startswith("license") or p.name.lower().startswith("licence") else 1, p.name.lower()))
    return files


def main():
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
    packages = parse_cargo_lock(CARGO_LOCK)
    out_path = ROOT / "codex-rs" / "THIRD-PARTY-LICENSES.txt"
    lines = []
    lines.append("This file aggregates license texts of third-party Rust crates bundled in codex binaries.\n")
    for p in sorted(packages, key=lambda x: (x["name"].lower(), x["version"])):
        cdir = find_crate_dir(cargo_home, p["name"], p["version"])
        if not cdir:
            continue
        lfiles = pick_license_files(cdir)
        if not lfiles:
            continue
        lines.append("-" * 79 + "\n")
        lines.append(f"{p['name']} {p['version']}\n")
        for lf in lfiles:
            try:
                text = lf.read_text(encoding="utf-8", errors="ignore")
            except Exception:
                continue
            lines.append(f"[ {lf.name} ]\n\n")
            lines.append(text.strip() + "\n\n")
    out_path.write_text("".join(lines), encoding="utf-8")
    print(f"Wrote {out_path}")


if __name__ == "__main__":
    main()

