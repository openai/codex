import type { AgentLoop } from "../../utils/agent/agent-loop.js";

import { Box, Text } from "ink";
import path from "node:path";
import React from "react";

export interface TerminalHeaderProps {
  terminalRows: number;
  version: string;
  PWD: string;
  model: string;
  provider?: string;
  approvalPolicy: string;
  colorsByPolicy: Record<string, string | undefined>;
  agent?: AgentLoop;
  initialImagePaths?: Array<string>;
  flexModeEnabled?: boolean;
  environment?: string;
}

const TerminalHeader: React.FC<TerminalHeaderProps> = ({
  terminalRows,
  version,
  PWD,
  model,
  provider = "openai",
  approvalPolicy,
  colorsByPolicy,
  agent,
  initialImagePaths,
  flexModeEnabled = false,
  environment,
}) => {
  const envName =
    environment ??
    (() => {
      // check for Windows
      if (process.platform === "win32") {
        const msystem = process.env["MSYSTEM"];
        // Check git bash
        if (msystem && msystem.toLowerCase().includes("mingw")) {
          return "Git Bash";
        }

        const keys = Object.keys(process.env).map((k) => k.toLowerCase());
        // Check pwsh vs powershell
        if (keys.includes("psedition")) {
          return "PowerShell Core";
        }
        // Check for PowerShell
        if (keys.includes("psmodulepath")) {
          return "PowerShell";
        }

        const comspec = process.env["ComSpec"];
        return comspec ? path.basename(comspec) : "cmd";
      }

      const shell = process.env["SHELL"];
      return shell ? path.basename(shell) : "sh";
    })();
  return (
    <>
      {terminalRows < 10 ? (
        // Compact header for small terminal windows
        <Text>
          ● Codex v{version} - {PWD} - {model} ({provider}) -{" "}
          <Text color={colorsByPolicy[approvalPolicy]}>{approvalPolicy}</Text>
          {flexModeEnabled ? " - flex-mode" : ""}
          {` - environment: ${envName}`}
        </Text>
      ) : (
        <>
          <Box borderStyle="round" paddingX={1} width={64}>
            <Text>
              ● OpenAI <Text bold>Codex</Text>{" "}
              <Text dimColor>
                (research preview) <Text color="blueBright">v{version}</Text>
              </Text>
            </Text>
          </Box>
          <Box
            borderStyle="round"
            borderColor="gray"
            paddingX={1}
            width={64}
            flexDirection="column"
          >
            <Text>
              localhost <Text dimColor>session:</Text>{" "}
              <Text color="magentaBright" dimColor>
                {agent?.sessionId ?? "<no-session>"}
              </Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> workdir: <Text bold>{PWD}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> model: <Text bold>{model}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> provider:{" "}
              <Text bold>{provider}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> environment:{" "}
              <Text bold>{envName}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> approval:{" "}
              <Text bold color={colorsByPolicy[approvalPolicy]}>
                {approvalPolicy}
              </Text>
            </Text>
            {flexModeEnabled && (
              <Text dimColor>
                <Text color="blueBright">↳</Text> flex-mode:{" "}
                <Text bold>enabled</Text>
              </Text>
            )}
            {initialImagePaths?.map((img, idx) => (
              <Text key={img ?? idx} color="gray">
                <Text color="blueBright">↳</Text> image:{" "}
                <Text bold>{path.basename(img)}</Text>
              </Text>
            ))}
          </Box>
        </>
      )}
    </>
  );
};

export default TerminalHeader;
