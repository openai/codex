import type { ExecInput } from "../../utils/agent/sandbox/interface.js";
import type { ToolHandler } from "../tool-registry.js";

import { handleExecCommand } from "../../utils/agent/handle-exec-command.js";

export const handleShellTool: ToolHandler = async (args, ctx) => {
  const { outputText, metadata, additionalItems } = await handleExecCommand(
    args as ExecInput,
    ctx.config,
    ctx.approvalPolicy,
    ctx.additionalWritableRoots,
    ctx.getCommandConfirmation,
    ctx.signal,
  );

  return {
    output: JSON.stringify({ output: outputText, metadata }),
    additionalItems,
  };
};
