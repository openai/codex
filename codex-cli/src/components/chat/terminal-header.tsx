import type { AgentLoop } from "../../utils/agent/agent-loop.js";

import { Box, Text } from "ink";
// import { Onboarding } from "../animations/onboarding.s";
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
  /** Optional dynamic cursor/indicator to show before the title (e.g. blinking >_) */
  blinkingIndicator?: string;
}

const TerminalHeader: React.FC<TerminalHeaderProps> = ({
  terminalRows,
  // keeping version for potential future use; prefix to silence unused warnings
  version: _version,
  PWD,
  model,
  provider = "openai",
  approvalPolicy,
  colorsByPolicy,
  agent,
  initialImagePaths,
  flexModeEnabled = false,
  blinkingIndicator,
}) => {
  {
    /* 
  const widenedOnboarding = React.useMemo(() => {
    try {
      return Onboarding.split("\n")
        .map((line) => line.split("").join(" "))
        .join("\n");
    } catch {
      return Onboarding;
    }
  }, []);
  */
  }
  return (
    <>
      {terminalRows < 10 ? (
        <Text>
          You are using <Text bold>OpenAI Codex</Text> in {PWD}
        </Text>
      ) : (
        <>
          <Box borderStyle="round" paddingX={1} width={64}>
            <Text color="gray">{(blinkingIndicator ?? ">_") + " "}</Text>
            <Text>
              You are using OpenAI <Text bold>Codex</Text> in{" "}
              <Text color="gray">{PWD}</Text>
            </Text>
          </Box>
          {/* 
          <Box marginTop={1} marginBottom={1} paddingX={1}>
            <Text dimColor>{widenedOnboarding}</Text>
          </Box>
          */}
          <Box
            paddingX={1}
            width={64}
            flexDirection="column"
            marginTop={1}
            marginBottom={1}
          >
            <Text dimColor>
              Describe a task to get started or try one of the following
              commands:
            </Text>
            <Text> </Text>
            <Text>
              <Text color="blueBright">/init</Text> - create an AGENTS.md file
              with instructions for Codex
            </Text>
            <Text>
              <Text color="blueBright">/status</Text> - show current session
              configuration and token usage
            </Text>
          </Box>
          {false && (
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
                <Text color="blueBright">↳</Text> workdir:{" "}
                <Text bold>{PWD}</Text>
              </Text>
              <Text dimColor>
                <Text color="blueBright">↳</Text> model:{" "}
                <Text bold>{model}</Text>
              </Text>
              <Text dimColor>
                <Text color="blueBright">↳</Text> provider:{" "}
                <Text bold>{provider}</Text>
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
          )}
        </>
      )}
    </>
  );
};

export default TerminalHeader;
