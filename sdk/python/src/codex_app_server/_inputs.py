from __future__ import annotations

from dataclasses import dataclass

from .models import JsonObject


@dataclass(slots=True)
class TextInput:
    text: str


@dataclass(slots=True)
class ImageInput:
    url: str


@dataclass(slots=True)
class LocalImageInput:
    path: str


@dataclass(slots=True)
class SkillInput:
    name: str
    path: str


@dataclass(slots=True)
class MentionInput:
    name: str
    path: str


InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem
RunInput = Input | str


def _to_wire_item(item: InputItem) -> JsonObject:
    if isinstance(item, TextInput):
        return {"type": "text", "text": item.text}
    if isinstance(item, ImageInput):
        return {"type": "image", "url": item.url}
    if isinstance(item, LocalImageInput):
        return {"type": "localImage", "path": item.path}
    if isinstance(item, SkillInput):
        return {"type": "skill", "name": item.name, "path": item.path}
    if isinstance(item, MentionInput):
        return {"type": "mention", "name": item.name, "path": item.path}
    raise TypeError(f"unsupported input item: {type(item)!r}")


def _to_wire_input(input: Input) -> list[JsonObject]:
    if isinstance(input, list):
        return [_to_wire_item(i) for i in input]
    return [_to_wire_item(input)]


def _normalize_run_input(input: RunInput) -> Input:
    if isinstance(input, str):
        return TextInput(input)
    return input
