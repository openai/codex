/* -------------------------------------------------------------------------- *
 * Tests for the HelpOverlay component
 *
 * The component displays help information with available commands and keyboard
 * shortcuts. It should be dismissible with the Escape key or 'q' key.
 * -------------------------------------------------------------------------- */

import { describe, it, expect, vi } from "vitest";
import { render } from "ink-testing-library";
import React from "react";
import HelpOverlay from "../src/components/help-overlay";

// ---------------------------------------------------------------------------
// Module mocks *must* be registered *before* the module under test is imported
// so that Vitest can replace the dependency during evaluation.
// ---------------------------------------------------------------------------

// Mock ink's useInput to capture keyboard handlers
let keyboardHandler: ((input: string, key: any) => void) | undefined;
vi.mock("ink", async () => {
  const actual = await vi.importActual("ink");
  return {
    ...actual,
    useInput: (handler: (input: string, key: any) => void) => {
      keyboardHandler = handler;
    },
  };
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("HelpOverlay", () => {
  describe("content display", () => {
    it("displays slash commands", () => {
      const { lastFrame } = render(<HelpOverlay onExit={vi.fn()} />);
      const frame = lastFrame()!;

      expect(frame).toContain("Available commands");
      expect(frame).toContain("/help");
      expect(frame).toContain("/model");
      expect(frame).toContain("/approval");
      expect(frame).toContain("/history");
      expect(frame).toContain("/clear");
      expect(frame).toContain("/clearhistory");
      expect(frame).toContain("/bug");
      expect(frame).toContain("/diff");
      expect(frame).toContain("/compact");
    });

    it("displays keyboard shortcuts", () => {
      const { lastFrame } = render(<HelpOverlay onExit={vi.fn()} />);
      const frame = lastFrame()!;

      expect(frame).toContain("Keyboard shortcuts");
      expect(frame).toContain("Enter");
      expect(frame).toContain("Ctrl+J");
      expect(frame).toContain("Up/Down");
      expect(frame).toContain("Esc");
      expect(frame).toContain("Ctrl+C");
    });

    it("displays exit instructions", () => {
      const { lastFrame } = render(<HelpOverlay onExit={vi.fn()} />);
      const frame = lastFrame()!;

      expect(frame).toContain("esc or q to close");
    });
  });

  describe("keyboard interaction", () => {
    it("handles escape key", () => {
      const onExit = vi.fn();
      render(<HelpOverlay onExit={onExit} />);

      keyboardHandler?.("", { escape: true });
      expect(onExit).toHaveBeenCalledTimes(1);
    });

    it("handles 'q' key", () => {
      const onExit = vi.fn();
      render(<HelpOverlay onExit={onExit} />);

      keyboardHandler?.("q", {});
      expect(onExit).toHaveBeenCalledTimes(1);
    });

    it("ignores other keys", () => {
      const onExit = vi.fn();
      render(<HelpOverlay onExit={onExit} />);

      // Try various other keys
      keyboardHandler?.("a", {});
      keyboardHandler?.("1", {});
      keyboardHandler?.("", { enter: true });
      keyboardHandler?.("", { upArrow: true });
      
      expect(onExit).not.toHaveBeenCalled();
    });
  });
});