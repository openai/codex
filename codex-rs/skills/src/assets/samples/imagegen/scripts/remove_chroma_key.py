#!/usr/bin/env python3
"""Remove a solid chroma-key background from an image.

This helper supports the imagegen skill's built-in-first transparent workflow:
generate an image on a flat key color, then convert that key color to alpha.
"""

from __future__ import annotations

import argparse
from io import BytesIO
from pathlib import Path
import re
from statistics import median
import sys
from typing import Iterable, Tuple


Color = Tuple[int, int, int]
KEY_DOMINANCE_THRESHOLD = 16.0
ALPHA_NOISE_FLOOR = 8


def _die(message: str, code: int = 1) -> None:
    print(f"Error: {message}", file=sys.stderr)
    raise SystemExit(code)


def _dependency_hint(package: str) -> str:
    return (
        "Activate the repo-selected environment first, then install it with "
        f"`uv pip install {package}`. If this repo uses a local virtualenv, start with "
        "`source .venv/bin/activate`; otherwise use this repo's configured shared fallback "
        "environment."
    )


def _load_pillow():
    try:
        from PIL import Image, ImageFilter
    except ImportError:
        _die(f"Pillow is required for chroma-key removal. {_dependency_hint('pillow')}")
    return Image, ImageFilter


def _parse_key_color(raw: str) -> Color:
    value = raw.strip()
    match = re.fullmatch(r"#?([0-9a-fA-F]{6})", value)
    if not match:
        _die("key color must be a hex RGB value like #00ff00.")
    hex_value = match.group(1)
    return (
        int(hex_value[0:2], 16),
        int(hex_value[2:4], 16),
        int(hex_value[4:6], 16),
    )


def _validate_args(args: argparse.Namespace) -> None:
    if args.tolerance < 0 or args.tolerance > 255:
        _die("--tolerance must be between 0 and 255.")
    if args.transparent_threshold < 0 or args.transparent_threshold > 255:
        _die("--transparent-threshold must be between 0 and 255.")
    if args.opaque_threshold < 0 or args.opaque_threshold > 255:
        _die("--opaque-threshold must be between 0 and 255.")
    if args.soft_matte and args.transparent_threshold >= args.opaque_threshold:
        _die("--transparent-threshold must be lower than --opaque-threshold.")
    if args.edge_feather < 0 or args.edge_feather > 64:
        _die("--edge-feather must be between 0 and 64.")
    if args.edge_contract < 0 or args.edge_contract > 16:
        _die("--edge-contract must be between 0 and 16.")

    src = Path(args.input)
    if not src.exists():
        _die(f"Input image not found: {src}")

    out = Path(args.out)
    if out.exists() and not args.force:
        _die(f"Output already exists: {out} (use --force to overwrite)")

    if out.suffix.lower() not in {".png", ".webp"}:
        _die("--out must end in .png or .webp so the alpha channel is preserved.")


def _channel_distance(a: Color, b: Color) -> int:
    return max(abs(a[0] - b[0]), abs(a[1] - b[1]), abs(a[2] - b[2]))


def _clamp_channel(value: float) -> int:
    return max(0, min(255, int(round(value))))


def _smoothstep(value: float) -> float:
    value = max(0.0, min(1.0, value))
    return value * value * (3.0 - 2.0 * value)


def _soft_alpha(distance: int, transparent_threshold: float, opaque_threshold: float) -> int:
    if distance <= transparent_threshold:
        return 0
    if distance >= opaque_threshold:
        return 255
    ratio = (float(distance) - transparent_threshold) / (
        opaque_threshold - transparent_threshold
    )
    return _clamp_channel(255.0 * _smoothstep(ratio))


def _dominance_alpha(rgb: Color, key: Color) -> int:
    spill_channels = _spill_channels(key)
    if not spill_channels:
        return 255

    channels = [float(value) for value in rgb]
    non_spill = [idx for idx in range(3) if idx not in spill_channels]
    key_strength = (
        min(channels[idx] for idx in spill_channels)
        if len(spill_channels) > 1
        else channels[spill_channels[0]]
    )
    non_key_strength = max((channels[idx] for idx in non_spill), default=0.0)
    dominance = key_strength - non_key_strength
    if dominance <= 0:
        return 255

    denominator = max(1.0, float(max(key)) - non_key_strength)
    alpha = 1.0 - min(1.0, dominance / denominator)
    return _clamp_channel(alpha * 255.0)


def _spill_channels(key: Color) -> list[int]:
    key_max = max(key)
    if key_max < 128:
        return []
    return [idx for idx, value in enumerate(key) if value >= key_max - 16 and value >= 128]


def _key_channel_dominance(rgb: Color, key: Color) -> float:
    spill_channels = _spill_channels(key)
    if not spill_channels:
        return 0.0

    channels = [float(value) for value in rgb]
    non_spill = [idx for idx in range(3) if idx not in spill_channels]
    key_strength = (
        min(channels[idx] for idx in spill_channels)
        if len(spill_channels) > 1
        else channels[spill_channels[0]]
    )
    non_key_strength = max((channels[idx] for idx in non_spill), default=0.0)
    return key_strength - non_key_strength


def _looks_key_colored(rgb: Color, key: Color, distance: int) -> bool:
    if distance <= 32:
        return True

    spill_channels = _spill_channels(key)
    if not spill_channels:
        return True

    return _key_channel_dominance(rgb, key) >= KEY_DOMINANCE_THRESHOLD


def _cleanup_spill(rgb: Color, key: Color, alpha: int = 255) -> Color:
    if alpha >= 252:
        return rgb

    spill_channels = _spill_channels(key)
    if not spill_channels:
        return rgb

    channels = [float(value) for value in rgb]
    non_spill = [idx for idx in range(3) if idx not in spill_channels]
    if non_spill:
        anchor = max(channels[idx] for idx in non_spill)
        cap = max(0.0, anchor - 1.0)
        for idx in spill_channels:
            if channels[idx] > cap:
                channels[idx] = cap

    return (
        _clamp_channel(channels[0]),
        _clamp_channel(channels[1]),
        _clamp_channel(channels[2]),
    )


def _alpha_for_pixels(
    pixels: Iterable[tuple[int, int, int, int]],
    *,
    key: Color,
    tolerance: int,
    spill_cleanup: bool,
    soft_matte: bool,
    transparent_threshold: float,
    opaque_threshold: float,
) -> tuple[list[tuple[int, int, int, int]], int]:
    output: list[tuple[int, int, int, int]] = []
    transparent = 0

    for red, green, blue, alpha in pixels:
        rgb = (red, green, blue)
        distance = _channel_distance(rgb, key)
        key_like = _looks_key_colored(rgb, key, distance)
        output_alpha = (
            min(
                _soft_alpha(distance, transparent_threshold, opaque_threshold),
                _dominance_alpha(rgb, key),
            )
            if soft_matte and key_like
            else (0 if distance <= tolerance else 255)
        )
        output_alpha = int(round(output_alpha * (alpha / 255.0)))
        if 0 < output_alpha <= ALPHA_NOISE_FLOOR:
            output_alpha = 0

        if output_alpha == 0:
            output.append((0, 0, 0, 0))
            transparent += 1
            continue

        if spill_cleanup and key_like:
            red, green, blue = _cleanup_spill(rgb, key, output_alpha)
        output.append((red, green, blue, output_alpha))

    return output, transparent


def _contract_alpha(image, pixels: int):
    if pixels == 0:
        return image

    _, ImageFilter = _load_pillow()
    alpha = image.split()[-1]
    for _ in range(pixels):
        alpha = alpha.filter(ImageFilter.MinFilter(3))
    image.putalpha(alpha)
    return image


def _apply_edge_feather(image, radius: float):
    if radius == 0:
        return image

    _, ImageFilter = _load_pillow()
    alpha = image.split()[-1]
    alpha = alpha.filter(ImageFilter.GaussianBlur(radius=radius))
    image.putalpha(alpha)
    return image


def _encode_image(image, output_format: str) -> bytes:
    out = BytesIO()
    image.save(out, format=output_format.upper())
    return out.getvalue()


def _get_pixels(image):
    getter = getattr(image, "get_flattened_data", None)
    return getter() if getter else image.getdata()


def _sample_border_key(image, mode: str) -> Color:
    rgb = image.convert("RGB")
    width, height = rgb.size
    samples: list[Color] = []

    if mode == "corners":
        patch = max(1, min(width, height, 12))
        boxes = [
            (0, 0, patch, patch),
            (width - patch, 0, width, patch),
            (0, height - patch, patch, height),
            (width - patch, height - patch, width, height),
        ]
        for left, top, right, bottom in boxes:
            for y in range(top, bottom):
                for x in range(left, right):
                    samples.append(rgb.getpixel((x, y)))
    else:
        band = max(1, min(width, height, 6))
        step = max(1, min(width, height) // 256)
        for x in range(0, width, step):
            for y in range(band):
                samples.append(rgb.getpixel((x, y)))
                samples.append(rgb.getpixel((x, height - 1 - y)))
        for y in range(0, height, step):
            for x in range(band):
                samples.append(rgb.getpixel((x, y)))
                samples.append(rgb.getpixel((width - 1 - x, y)))

    if not samples:
        _die("Could not sample background key color from image border.")

    return (
        int(round(median(sample[0] for sample in samples))),
        int(round(median(sample[1] for sample in samples))),
        int(round(median(sample[2] for sample in samples))),
    )


def _remove_chroma_key(args: argparse.Namespace) -> None:
    Image, _ = _load_pillow()
    src = Path(args.input)
    out = Path(args.out)

    with Image.open(src) as image:
        rgba = image.convert("RGBA")
    key = (
        _sample_border_key(rgba, args.auto_key)
        if args.auto_key != "none"
        else _parse_key_color(args.key_color)
    )

    pixels, transparent = _alpha_for_pixels(
        _get_pixels(rgba),
        key=key,
        tolerance=args.tolerance,
        spill_cleanup=args.spill_cleanup,
        soft_matte=args.soft_matte,
        transparent_threshold=args.transparent_threshold,
        opaque_threshold=args.opaque_threshold,
    )
    rgba.putdata(pixels)
    rgba = _contract_alpha(rgba, args.edge_contract)
    rgba = _apply_edge_feather(rgba, args.edge_feather)

    alpha_values = [pixel[3] for pixel in _get_pixels(rgba)]
    total = len(alpha_values)
    transparent_after = sum(1 for alpha in alpha_values if alpha == 0)
    partial_after = sum(1 for alpha in alpha_values if 0 < alpha < 255)

    out.parent.mkdir(parents=True, exist_ok=True)
    output_format = "PNG" if out.suffix.lower() == ".png" else "WEBP"
    out.write_bytes(_encode_image(rgba, output_format))

    print(f"Wrote {out}")
    print(f"Key color: #{key[0]:02x}{key[1]:02x}{key[2]:02x}")
    print(f"Transparent pixels: {transparent_after}/{total}")
    print(f"Partially transparent pixels: {partial_after}/{total}")
    if transparent == 0:
        print("Warning: no pixels matched the key color before feathering.", file=sys.stderr)


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Remove a solid chroma-key background and write an image with alpha."
    )
    parser.add_argument("--input", required=True, help="Input image path.")
    parser.add_argument("--out", required=True, help="Output .png or .webp path.")
    parser.add_argument(
        "--key-color",
        default="#00ff00",
        help="Hex RGB key color to remove, for example #00ff00.",
    )
    parser.add_argument(
        "--tolerance",
        type=int,
        default=12,
        help="Hard-key per-channel tolerance for matching the key color, 0-255.",
    )
    parser.add_argument(
        "--auto-key",
        choices=["none", "corners", "border"],
        default="none",
        help="Sample the key color from image corners or border instead of --key-color.",
    )
    parser.add_argument(
        "--soft-matte",
        action="store_true",
        help="Use a smooth alpha ramp between transparent and opaque thresholds.",
    )
    parser.add_argument(
        "--transparent-threshold",
        type=float,
        default=12.0,
        help="Soft-matte distance at or below which pixels become fully transparent.",
    )
    parser.add_argument(
        "--opaque-threshold",
        type=float,
        default=96.0,
        help="Soft-matte distance at or above which pixels become fully opaque.",
    )
    parser.add_argument(
        "--edge-feather",
        type=float,
        default=0.0,
        help="Optional alpha blur radius for softened edges, 0-64.",
    )
    parser.add_argument(
        "--edge-contract",
        type=int,
        default=0,
        help="Shrink the visible alpha matte by this many pixels before feathering.",
    )
    parser.add_argument(
        "--spill-cleanup",
        dest="spill_cleanup",
        action="store_true",
        help="Reduce obvious key-color spill on opaque pixels.",
    )
    parser.add_argument(
        "--despill",
        dest="spill_cleanup",
        action="store_true",
        help="Alias for --spill-cleanup; decontaminate key-color edge spill.",
    )
    parser.add_argument("--force", action="store_true", help="Overwrite an existing output file.")
    return parser


def main() -> None:
    parser = _build_parser()
    args = parser.parse_args()
    _validate_args(args)
    _remove_chroma_key(args)


if __name__ == "__main__":
    main()
