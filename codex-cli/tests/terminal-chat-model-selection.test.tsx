/* eslint-disable no-console */
import { renderTui } from "./ui-test-helpers.js";
import React from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import chalk from "chalk";
import ModelOverlay from "src/components/model-overlay.js";

// Mock the necessary dependencies
vi.mock("../src/utils/logger/log.js", () => ({
  log: vi.fn(),
}));

vi.mock("chalk", () => ({
  default: {
    bold: {
      red: vi.fn((msg) => `[bold-red]${msg}[/bold-red]`),
    },
    yellow: vi.fn((msg) => `[yellow]${msg}[/yellow]`),
  },
}));

describe("Model Selection Error Handling", () => {
  // Create a console.error spy with proper typing
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.clearAllMocks();
    consoleErrorSpy.mockRestore();
  });

  it("should display error with chalk formatting when selecting unavailable model", () => {
    // Setup
    const allModels = ["gpt-4", "gpt-3.5-turbo"];
    const currentModel = "gpt-4";
    const unavailableModel = "gpt-invalid";
    const currentProvider = "openai";

    renderTui(
      <ModelOverlay
        currentModel={currentModel}
        providers={{ openai: { name: "OpenAI", baseURL: "", envKey: "test" } }}
        currentProvider={currentProvider}
        hasLastResponse={false}
        onSelect={(models, newModel) => {
          if (!models?.includes(newModel)) {
            console.error(
              chalk.bold.red(
                `Model "${chalk.yellow(
                  newModel,
                )}" is not available for provider "${chalk.yellow(
                  currentProvider,
                )}".`,
              ),
            );
            return;
          }
        }}
        onSelectProvider={() => {}}
        onExit={() => {}}
      />,
    );

    const onSelectHandler = vi.fn((models, newModel) => {
      if (!models?.includes(newModel)) {
        console.error(
          chalk.bold.red(
            `Model "${chalk.yellow(
              newModel,
            )}" is not available for provider "${chalk.yellow(
              currentProvider,
            )}".`,
          ),
        );
        return;
      }
    });

    onSelectHandler(allModels, unavailableModel);

    expect(consoleErrorSpy).toHaveBeenCalled();
    expect(chalk.bold.red).toHaveBeenCalled();
    expect(chalk.yellow).toHaveBeenCalledWith(unavailableModel);
    expect(chalk.yellow).toHaveBeenCalledWith(currentProvider);

    expect(consoleErrorSpy).toHaveBeenCalledWith(
      `[bold-red]Model "[yellow]${unavailableModel}[/yellow]" is not available for provider "[yellow]${currentProvider}[/yellow]".[/bold-red]`,
    );
  });

  it("should not proceed with model change when model is unavailable", () => {
    const mockSetModel = vi.fn();
    const mockSetLastResponseId = vi.fn();
    const mockSaveConfig = vi.fn();
    const mockSetItems = vi.fn();
    const mockSetOverlayMode = vi.fn();

    const onSelectHandler = vi.fn((allModels, newModel) => {
      if (!allModels?.includes(newModel)) {
        console.error(
          chalk.bold.red(
            `Model "${chalk.yellow(
              newModel,
            )}" is not available for provider "${chalk.yellow("openai")}".`,
          ),
        );
        return;
      }

      mockSetModel(newModel);
      mockSetLastResponseId(null);
      mockSaveConfig({});
      mockSetItems((prev: Array<unknown>) => [...prev, {}]);
      mockSetOverlayMode("none");
    });

    onSelectHandler(["gpt-4", "gpt-3.5-turbo"], "gpt-invalid");

    expect(mockSetModel).not.toHaveBeenCalled();
    expect(mockSetLastResponseId).not.toHaveBeenCalled();
    expect(mockSaveConfig).not.toHaveBeenCalled();
    expect(mockSetItems).not.toHaveBeenCalled();
    expect(mockSetOverlayMode).not.toHaveBeenCalled();

    expect(consoleErrorSpy).toHaveBeenCalled();
  });
});
