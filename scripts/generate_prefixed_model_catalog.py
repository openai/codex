#!/usr/bin/env python3
"""Fetch a Codex models.json from a public GitHub tag and prefix model slugs.

Example:
  python scripts/generate_prefixed_model_catalog.py \
    --tag 0.104.0 \
    --prefix custom/ \
    --out ./models.custom.json

This custom models.json can be used to override the default models.json used
by Codex via the `model_catalog_json` config option.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path


REPO = "openai/codex"
RAW_MODELS_URL = "https://raw.githubusercontent.com/{repo}/{tag}/codex-rs/core/models.json"
HTTP_TIMEOUT_SECONDS = 15.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Fetch codex-rs/core/models.json for a public openai/codex tag and prefix model slugs."
        )
    )
    parser.add_argument("--tag", required=True, help="Git tag/branch/commit to fetch")
    parser.add_argument(
        "--prefix",
        required=True,
        help="Prefix to add to each model slug (e.g. 'custom/' or 'gateway/')",
    )
    parser.add_argument("--out", required=True, help="Output path for the generated JSON")
    return parser.parse_args()


def fetch_catalog(repo: str, tag: str, timeout_seconds: float) -> tuple[str, dict]:
    url = RAW_MODELS_URL.format(repo=repo, tag=tag)
    request = urllib.request.Request(
        url,
        headers={"User-Agent": "codex-model-catalog-prefixer/1"},
    )
    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        payload = response.read()
    return url, json.loads(payload.decode("utf-8"))


def normalize_tag(tag: str) -> str:
    if tag.startswith("rust-"):
        return tag
    if tag.startswith("v"):
        return f"rust-{tag}"
    return f"rust-v{tag}"


def prefixed(prefix: str, value: str) -> str:
    if value.startswith(prefix):
        return value
    return f"{prefix}{value}"


def rewrite_catalog(catalog: dict, prefix: str) -> dict[str, int]:
    models = catalog.get("models")
    if not isinstance(models, list):
        raise ValueError("expected JSON object with a top-level 'models' list")

    slug_updates = 0
    upgrade_updates = 0

    for idx, model in enumerate(models):
        if not isinstance(model, dict):
            raise ValueError(f"models[{idx}] is not an object")

        slug = model.get("slug")
        if not isinstance(slug, str):
            raise ValueError(f"models[{idx}].slug is missing or not a string")

        new_slug = prefixed(prefix, slug)
        if new_slug != slug:
            model["slug"] = new_slug
            slug_updates += 1

        upgrade = model.get("upgrade")
        if isinstance(upgrade, dict):
            upgrade_model = upgrade.get("model")
            if isinstance(upgrade_model, str):
                new_upgrade_model = prefixed(prefix, upgrade_model)
                if new_upgrade_model != upgrade_model:
                    upgrade["model"] = new_upgrade_model
                    upgrade_updates += 1

    return {
        "models": len(models),
        "slug_updates": slug_updates,
        "upgrade_model_updates": upgrade_updates,
    }


def main() -> int:
    args = parse_args()
    if not args.prefix:
        print("error: --prefix must not be empty", file=sys.stderr)
        return 2
    tag = normalize_tag(args.tag)

    try:
        source_url, catalog = fetch_catalog(REPO, tag, HTTP_TIMEOUT_SECONDS)
        stats = rewrite_catalog(catalog, args.prefix)
    except urllib.error.HTTPError as err:
        print(f"error: failed to fetch models.json ({err.code}): {err.url}", file=sys.stderr)
        return 1
    except urllib.error.URLError as err:
        print(f"error: network error while fetching models.json: {err}", file=sys.stderr)
        return 1
    except json.JSONDecodeError as err:
        print(f"error: fetched file is not valid JSON: {err}", file=sys.stderr)
        return 1
    except ValueError as err:
        print(f"error: unexpected models.json shape: {err}", file=sys.stderr)
        return 1

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")

    print(f"Fetched: {source_url}")
    print(f"Wrote:   {out_path}")
    print(
        "Models: {models} (updated {slug_updates}); upgrade.model updated: "
        "{upgrade_model_updates}".format(**stats)
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
