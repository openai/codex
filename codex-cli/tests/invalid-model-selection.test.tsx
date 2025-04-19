import * as modelUtils from "../src/utils/model-utils.js";
import { vi, describe, test, expect, beforeEach, afterEach } from "vitest";

vi.mock("../src/utils/agent/agent-loop.js", () => ({
  AgentLoop: vi.fn().mockImplementation(() => ({
    cancel: vi.fn(),
    terminate: vi.fn(),
    run: vi.fn(),
  })),
}));

vi.mock("../src/utils/model-utils.js", () => ({
  getAvailableModels: vi.fn(),
  RECOMMENDED_MODELS: ["gpt-4o", "gpt-4-turbo"],
}));

describe("ModelOverlay component", () => {
  let availableModels: Array<string>;

  beforeEach(() => {
    availableModels = ["gpt-4o", "gpt-4-turbo", "gpt-3.5-turbo"];

    vi.mocked(modelUtils.getAvailableModels).mockResolvedValue(availableModels);

    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  test("should display error message when model selection is not available", async () => {
    const mockAgent = {
      cancel: vi.fn(),
    };

    const mockSetModel = vi.fn();
    const mockSetLastResponseId = vi.fn();
    const mockSetItems = vi.fn();
    const mockSetOverlayMode = vi.fn();

    const handleModelSelect = (newModel: string) => {
      mockAgent.cancel();

      if (!availableModels.includes(newModel)) {
        mockSetOverlayMode("none");
        return;
      }

      mockSetModel(newModel);
      mockSetLastResponseId((prev: string | null) =>
        prev && newModel !== "gpt-4o" ? null : prev,
      );

      mockSetItems((prev: Array<any>) => [
        ...prev,
        {
          id: expect.stringContaining("switch-model-"),
          type: "message",
          role: "system",
          content: [
            {
              type: "input_text",
              text: `Switched model to ${newModel}`,
            },
          ],
        },
      ]);

      mockSetOverlayMode("none");
    };

    // test case: non-existent model
    handleModelSelect("nonexistent-model");

    expect(mockAgent.cancel).toHaveBeenCalled();
    expect(mockSetOverlayMode).toHaveBeenCalledWith("none");
    expect(mockSetModel).not.toHaveBeenCalled();

    // test case: valid model
    handleModelSelect("gpt-3.5-turbo");

    expect(mockSetModel).toHaveBeenCalledWith("gpt-3.5-turbo");
    expect(mockSetLastResponseId).toHaveBeenCalled();
    expect(mockSetItems).toHaveBeenCalled();
    expect(mockSetOverlayMode).toHaveBeenCalledWith("none");
  });
});
