#!/usr/bin/env python3
"""Minimal Windows helper script with optional voice support.

This script demonstrates how the Codex CLI could ask users questions on Windows.
It displays questions near the mouse pointer and optionally uses text-to-speech.
When the user presses Ctrl, an input window appears. Pressing Alt attempts to
capture a spoken answer. If the window is closed or Escape is pressed, a default
answer is returned.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import threading
import tkinter as tk
from tkinter import simpledialog

try:
    import pyautogui  # type: ignore
except Exception:  # pragma: no cover - optional dependency
    pyautogui = None

try:
    import pyttsx3  # type: ignore
except Exception:  # pragma: no cover - optional dependency
    pyttsx3 = None

try:
    import speech_recognition as sr  # type: ignore
except Exception:  # pragma: no cover - optional dependency
    sr = None


def speak(text: str) -> None:
    if pyttsx3 is None:
        return
    engine = pyttsx3.init()
    engine.say(text)
    engine.runAndWait()


def record_voice() -> str | None:
    if sr is None:
        return None
    recognizer = sr.Recognizer()
    with sr.Microphone() as source:
        try:
            audio = recognizer.listen(source, timeout=5)
            return recognizer.recognize_google(audio)
        except Exception:
            return None


def open_editor(initial: str = "") -> str:
    """Open the micro editor and return the resulting text."""
    with open("tmp_input.txt", "w", encoding="utf-8") as f:
        f.write(initial)
    subprocess.call(["micro", "tmp_input.txt"])
    with open("tmp_input.txt", "r", encoding="utf-8") as f:
        return f.read().strip()


def ask_question(question: str, with_voice: bool = False, default: str = "") -> str:
    if with_voice:
        threading.Thread(target=speak, args=(question,), daemon=True).start()

    root = tk.Tk()
    root.attributes("-topmost", True)
    root.overrideredirect(True)
    root.configure(bg="black")

    text_var = tk.StringVar(value="")
    label = tk.Label(root, textvariable=text_var, fg="white", bg="black")
    label.pack()

    if pyautogui:
        x, y = pyautogui.position()
        root.geometry(f"+{x}+{y}")

    def on_key(event: tk.Event[tk.Misc]) -> None:
        nonlocal answer
        if event.keysym == "Control_L" or event.keysym == "Control_R":
            answer = simpledialog.askstring("Answer", question, parent=root) or default
            root.destroy()
        elif event.keysym == "Alt_L" or event.keysym == "Alt_R":
            voice = record_voice()
            answer = voice if voice else default
            root.destroy()
        elif event.keysym == "Escape":
            answer = default
            root.destroy()

    root.bind_all("<Key>", on_key)

    for ch in question + " (+ctrl/+alt)":
        text_var.set(text_var.get() + ch)
        root.update()
        root.after(50)

    answer = default
    root.mainloop()
    return answer


def main() -> None:
    parser = argparse.ArgumentParser(description="Windows helper for Codex")
    parser.add_argument("question", help="Question to ask")
    parser.add_argument("--voice", action="store_true", help="Enable text-to-speech")
    args = parser.parse_args()

    resp = ask_question(args.question, with_voice=args.voice)
    print("Answer:", resp)


if __name__ == "__main__":
    main()
