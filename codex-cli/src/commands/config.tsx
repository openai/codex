/**
 * Interactive configuration command for Codex CLI.
 * 
 * This module provides a command to launch an interactive configuration
 * interface using Ink components.
 */

import React from "react";
import { render } from "ink";
import { ConfigMenu } from "../components/config/config-menu.js";

/**
 * Run the interactive configuration command.
 * 
 * This function renders the configuration UI and sets up
 * a global exit handler to allow components to exit the process.
 */
export async function runConfigCommand(): Promise<void> {
  // Create a promise that will be resolved when the config UI should exit
  const exitPromise = new Promise<void>((resolve) => {
    // Set a global function that components can call to exit
    (global as any).__configExit = () => {
      resolve();
    };
  });

  // Render the config menu
  const { unmount } = render(<ConfigMenu />);

  // Wait for the exit signal
  await exitPromise;

  // Clean up
  unmount();
  delete (global as any).__configExit;
}
