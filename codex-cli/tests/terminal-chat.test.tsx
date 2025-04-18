import React from "react";
import { render } from "@testing-library/react";
import TerminalChat from "../src/components/chat/terminal-chat";
import { AppConfig } from "../src/utils/config";

// Mock getAvailableModels to control the available models in the test
jest.mock("../src/utils/model-utils", () => ({
  getAvailableModels: () => Promise.resolve(["gpt-4", "gpt-3.5"]),
}));

describe("TerminalChat model validation", () => {
  it("should display a warning if the configured model is unavailable", async () => {
    // Set a model that is NOT in the available list
    const config: AppConfig = { model: "gpt-unicorn" } as any;
    const { findByText } = render(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );
    expect(
      await findByText(
        /Warning: model "gpt-unicorn" is not in the list of available models/i,
      ),
    ).toBeInTheDocument();
  });

  it("should NOT display a warning if the configured model is available", async () => {
    // Set a model that IS in the available list
    const config: AppConfig = { model: "gpt-3.5" } as any;
    const { queryByText } = render(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );
    // Give React time to update
    await new Promise((res) => setTimeout(res, 100));
    expect(
      queryByText(
        /Warning: model "gpt-3.5" is not in the list of available models/i,
      ),
    ).toBeNull();
  });
});
