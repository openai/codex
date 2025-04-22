/*
 * Regression test – ensure that the TypeaheadOverlay passes the *complete*
 * list of items down to <SelectInput>.  This guarantees that users can scroll
 * through the full set instead of being limited to the hard‑coded "limit"
 * slice that is only meant to control how many rows are visible at once.
 */

import * as React from "react";
import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
//  Mock <select-input> so we can capture the props that TypeaheadOverlay
//  forwards without rendering the real component (which would require a full
//  Ink TTY environment).
// ---------------------------------------------------------------------------

let receivedItems: Array<{ label: string; value: string }> | null = null;
vi.mock("../src/components/select-input/select-input.js", () => {
  return {
    default: (props: any) => {
      receivedItems = props.items;
      return null; // Do not render anything – we only care about the props
    },
  };
});

// Ink's <TextInput> toggles raw‑mode which calls .ref() / .unref() on stdin.
// The test environment's mock streams don't implement those methods, so we
// polyfill them to no-ops on the prototype *before* the component tree mounts.
import { EventEmitter } from "node:events";
if (!(EventEmitter.prototype as any).ref) {
  (EventEmitter.prototype as any).ref = () => {};
  (EventEmitter.prototype as any).unref = () => {};
}

import type { TypeaheadItem } from "../src/components/typeahead-overlay.js";
import TypeaheadOverlay from "../src/components/typeahead-overlay.js";

import { renderTui } from "./ui-test-helpers.js";

describe("TypeaheadOverlay – scrolling capability", () => {
  it("passes the full item list to <SelectInput> so users can scroll beyond the visible limit", async () => {
    const ITEMS: Array<TypeaheadItem> = Array.from({ length: 20 }, (_, i) => ({
      label: `model-${i + 1}`,
      value: `model-${i + 1}`,
    }));

    // Sanity – reset capture before rendering
    receivedItems = null;

    const { flush, cleanup } = renderTui(
      React.createElement(TypeaheadOverlay, {
        title: "Test",
        initialItems: ITEMS,
        limit: 5, // visible rows – should *not* limit the underlying list
        onSelect: () => {},
        onExit: () => {},
      }),
    );

    await flush(); // allow first render to complete

    expect(receivedItems).not.toBeNull();
    expect((receivedItems ?? []).length).toBe(ITEMS.length);

    cleanup();
  });
});
