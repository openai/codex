import type { CommandConfirmation } from "../../utils/agent/agent-loop.js";
import type { ToolCallResult } from "../../utils/agent/tool-executor";
import type { AppConfig } from "../../utils/config.js";
import type { ApplyPatchCommand, ApprovalPolicy } from "@lib/approvals.js";
import type { ColorName } from "chalk";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";
import type { ReviewDecision } from "src/utils/agent/review.ts";

import TerminalChatInput from "./terminal-chat-input.js";
import { TerminalChatToolCallCommand } from "./terminal-chat-tool-call-item.js";
import { TerminalChatToolExecutionItem } from "./terminal-chat-tool-execution-item";
import {
  calculateContextPercentRemaining,
  uniqueById,
} from "./terminal-chat-utils.js";
import TerminalMessageHistory from "./terminal-message-history.js";
import { useConfirmation } from "../../hooks/use-confirmation.js";
import { useTerminalSize } from "../../hooks/use-terminal-size.js";
import { AgentLoop } from "../../utils/agent/agent-loop.js";
import { log, isLoggingEnabled } from "../../utils/agent/log.js";
import { createInputItem } from "../../utils/input-utils.js";
import { McpManager } from "../../utils/mcp-manager"; // Use McpManager
import { getAvailableModels } from "../../utils/model-utils.js";
import { CLI_VERSION } from "../../utils/session.js";
import { shortCwd } from "../../utils/short-path.js";
import { saveRollout } from "../../utils/storage/save-rollout.js";
import ApprovalModeOverlay from "../approval-mode-overlay.js";
import HelpOverlay from "../help-overlay.js";
import HistoryOverlay from "../history-overlay.js";
import ModelOverlay from "../model-overlay.js";
import { formatCommandForDisplay } from "@lib/format-command.js";
import { Box, Text } from "ink";
import React, { useEffect, useMemo, useState, useRef, useReducer } from "react"; // Added useRef, useReducer
import { inspect } from "util";

type Props = {
  config: AppConfig;
  prompt?: string;
  imagePaths?: Array<string>;
  approvalPolicy: ApprovalPolicy;
  fullStdout: boolean;
  withMcpTools?: boolean;
};

const colorsByPolicy: Record<ApprovalPolicy, ColorName | undefined> = {
  "suggest": undefined,
  "auto-edit": "greenBright",
  "full-auto": "green",
};

export default function TerminalChat({
  config,
  prompt: _initialPrompt,
  imagePaths: _initialImagePaths,
  approvalPolicy: initialApprovalPolicy,
  fullStdout,
  withMcpTools = true,
}: Props): React.ReactElement {
  const [model, setModel] = useState<string>(config.model);
  const [lastResponseId, setLastResponseId] = useState<string | null>(null);
  const [items, setItems] = useState<Array<ResponseItem>>([]);
  const [loading, setLoading] = useState<boolean>(false);
  // Allow switching approval modes at runtime via an overlay.
  const [approvalPolicy, setApprovalPolicy] = useState<ApprovalPolicy>(
    initialApprovalPolicy,
  );
  const [thinkingSeconds, setThinkingSeconds] = useState(0);
  const [toolResults, setToolResults] = useState<Array<ToolCallResult>>([]);
  const { requestConfirmation, confirmationPrompt, submitConfirmation } =
    useConfirmation();
  const [overlayMode, setOverlayMode] = useState<
    "none" | "history" | "model" | "approval" | "help" | "mcp"
  >("none");

  // Create MCP Manager instance if enabled - This instance is *only* for displaying tools info
  // AgentLoop will create its own internal instance.
  const [mcpManagerForDisplay] = useState<McpManager | undefined>(() =>
    withMcpTools
      ? new McpManager({ debugMode: Boolean(process.env["MCP_DEBUG"]) })
      : undefined,
  );

  const [initialPrompt, setInitialPrompt] = useState(_initialPrompt);
  const [initialImagePaths, setInitialImagePaths] =
    useState(_initialImagePaths);

  const PWD = React.useMemo(() => shortCwd(), []);

  // Keep a single AgentLoop instance alive across renders;
  // recreate only when model/instructions/approvalPolicy change.
  const agentRef = useRef<AgentLoop>(); // Changed from React.useRef
  const [, forceUpdate] = useReducer((c) => c + 1, 0); // Changed from React.useReducer

  // Track MCP Manager initialization state (for the display instance)
  const [mcpManagerInitialized, setMcpManagerInitialized] = useState(
    !withMcpTools,
  ); // Initialize true if not using MCP

  // Initialize MCP Manager (for display) if enabled
  useEffect(() => {
    if (!mcpManagerForDisplay) {
      return;
    } // Use the display instance

    const initManager = async () => {
      try {
        // Add timeout for initialization
        const initPromise = mcpManagerForDisplay.initialize(); // Use the display instance
        const timeoutPromise = new Promise((_, reject) => {
          setTimeout(
            () =>
              reject(
                new Error("MCP initialization timed out after 10 seconds"),
              ),
            10000,
          );
        });

        await Promise.race([initPromise, timeoutPromise]);
        const availableTools = mcpManagerForDisplay.getAvailableTools(); // Use the display instance
        log(
          `MCP Manager (for display) initialized with ${availableTools.length} tools`,
        );

        // Add a system message showing available tools
        if (availableTools.length > 0) {
          const toolCategories: Record<string, Array<string>> = {};

          // Group tools by server/category
          for (const tool of availableTools) {
            const parts = tool.name.split("__");
            if (parts.length === 3) {
              // Format: mcp__server__tool
              const mcpServerName = parts[1] || "unknown";
              // Initialize the category if it doesn't exist
              if (!toolCategories[serverName]) {
                toolCategories[serverName] = [];
              }
              if (parts[2]) {
                toolCategories[serverName].push(parts[2]); // Just the tool name without prefix
              }
            } else {
              // Non-standard format, just use the whole name
              const otherCategory = "other";
              if (!toolCategories[otherCategory]) {
                toolCategories[otherCategory] = [];
              }
              toolCategories[otherCategory].push(tool.name);
            }
          }

          // Create a formatted message of tools by category
          // Create a simpler message that doesn't focus on tools
          let toolsMessage =
            "💡 I can now help with a wider range of tasks including:\n";

          // Map capabilities rather than tool names
          const capabilities = [];

          // Safely check for fetch capabilities
          if (
            Object.keys(toolCategories).some((cat) => {
              return (
                cat.includes("fetch") ||
                (toolCategories[cat] &&
                  Array.isArray(toolCategories[cat]) &&
                  toolCategories[cat].some(
                    (t) => typeof t === "string" && t.includes("fetch"),
                  ))
              );
            })
          ) {
            capabilities.push(
              "• Accessing web content and current information",
            );
          }

          // Safely check for vision capabilities
          if (
            Object.keys(toolCategories).some((cat) => {
              return (
                cat.includes("vision") ||
                cat.includes("image") ||
                (toolCategories[cat] &&
                  Array.isArray(toolCategories[cat]) &&
                  toolCategories[cat].some(
                    (t) =>
                      typeof t === "string" &&
                      (t.includes("vision") || t.includes("image")),
                  ))
              );
            })
          ) {
            capabilities.push("• Analyzing images and visual content");
          }

          // Add a generic capability for other tool types
          if (
            Object.keys(toolCategories).length > 0 &&
            capabilities.length < 2
          ) {
            // Avoid redundancy if fetch/vision already listed
            capabilities.push("• Finding information from specialized sources");
          }

          // Add all capabilities
          toolsMessage += capabilities.join("\n");

          // Add a friendly instruction that doesn't mention tools
          toolsMessage +=
            "\n\nJust ask your questions naturally - I'll handle the rest!";

          // Find the fetch tool if available
          const fetchToolName = "fetch";
          const fetchTool = availableTools.find((t: { name: string }) =>
            t.name.includes(fetchToolName),
          );

          // Create a properly typed system message
          const createSystemMessage = (
            id: string,
            text: string,
          ): ResponseItem => ({
            id,
            type: "message",
            role: "system",
            status: "completed" as const, // Status for ResponseOutputMessage
            content: [
              {
                type: "input_text",
                text,
              },
            ],
          });

          const messages: Array<ResponseItem> = [
            createSystemMessage(`mcp-tools-${Date.now()}`, toolsMessage),
          ];

          // Add a more minimal, elegant message
          if (fetchTool) {
            messages.push(
              createSystemMessage(
                `mcp-examples-${Date.now()}`,
                `Try asking about:
• "What's happening in the news today?"
• "Show me the weather for San Francisco"
• "What's on the React homepage?"`,
              ),
            );
          }

          setItems((prev) => [...prev, ...messages]);
        }

        // Mark manager as initialized
        setMcpManagerInitialized(true);
      } catch (err) {
        log(`Error initializing Mcp manager (for display): ${err}`);
        // Even if there's an error, mark as initialized so we can create the agent
        setMcpManagerInitialized(true);
      }
    };

    initManager();

    // Clean up on unmount
    return () => {
      mcpManagerForDisplay?.dispose(); // Use the display instance
    };
  }, [mcpManagerForDisplay]); // Depend on the display instance

  // ────────────────────────────────────────────────────────────────
  // DEBUG: log every render w/ key bits of state
  // ────────────────────────────────────────────────────────────────
  if (isLoggingEnabled()) {
    log(
      `render – agent? ${Boolean(agentRef.current)} loading=${loading} items=${
        items.length
      }`,
    );
  }

  // Effect to create/recreate AgentLoop when dependencies change
  useEffect(() => {
    // Wait for MCP Manager (for display) to be initialized if MCP tools are enabled
    // This ensures the initial tool message is shown before the agent starts.
    if (withMcpTools && !mcpManagerInitialized) {
      log(
        "Waiting for Mcp manager (for display) initialization before creating AgentLoop",
      );
      return;
    }

    if (isLoggingEnabled()) {
      log("creating NEW AgentLoop");
      log(
        `model=${model} instructions=${Boolean(
          config.instructions,
        )} approvalPolicy=${approvalPolicy}`,
      );
      // AgentLoop now initializes its own McpManager, no need to log tools here
    }

    // Tear down any existing loop before creating a new one
    agentRef.current?.terminate();

    agentRef.current = new AgentLoop({
      model,
      config,
      instructions: config.instructions,
      approvalPolicy,
      onLastResponseId: setLastResponseId,
      onItem: (item) => {
        log(`onItem: ${JSON.stringify(item)}`);
        setItems((prev) => {
          const updated = uniqueById([...prev, item as ResponseItem]);
          saveRollout(updated);
          return updated;
        });
      },
      onLoading: setLoading,
      onToolCall: (result) => {
        // Handle tool call results
        log(`onToolCall: ${JSON.stringify(result)}`);
        setToolResults((prev) => [...prev, result]);
      },
      getCommandConfirmation: async (
        command: Array<string>,
        applyPatch: ApplyPatchCommand | undefined,
      ): Promise<CommandConfirmation> => {
        log(`getCommandConfirmation: ${command}`);
        const commandForDisplay = formatCommandForDisplay(command);
        const { decision: review, customDenyMessage } =
          await requestConfirmation(
            <TerminalChatToolCallCommand
              commandForDisplay={commandForDisplay}
            />,
          );
        return { review, customDenyMessage, applyPatch };
      },
    });

    // force a render so JSX below can "see" the freshly created agent
    forceUpdate();

    if (isLoggingEnabled()) {
      log(`AgentLoop created: ${inspect(agentRef.current, { depth: 1 })}`);
    }

    return () => {
      if (isLoggingEnabled()) {
        log("terminating AgentLoop");
      }
      agentRef.current?.terminate();
      agentRef.current = undefined;
      forceUpdate(); // re‑render after teardown too
    };
  }, [
    model,
    config,
    approvalPolicy,
    // mcpManagerInitialized gates the creation, ensuring display init happens first
    mcpManagerInitialized,
    withMcpTools, // Recreate if MCP tools are toggled
    requestConfirmation, // Include requestConfirmation from useConfirmation hook
  ]);

  // whenever loading starts/stops, reset or start a timer — but pause the
  // timer while a confirmation overlay is displayed so we don't trigger a
  // re‑render every second during apply_patch reviews.
  useEffect(() => {
    let handle: ReturnType<typeof setInterval> | null = null;
    // Only tick the "thinking…" timer when the agent is actually processing
    // a request *and* the user is not being asked to review a command.
    if (loading && confirmationPrompt == null) {
      setThinkingSeconds(0);
      handle = setInterval(() => {
        setThinkingSeconds((s) => s + 1);
      }, 1000);
    } else {
      if (handle) {
        clearInterval(handle);
      }
      setThinkingSeconds(0);
    }
    return () => {
      if (handle) {
        clearInterval(handle);
      }
    };
  }, [loading, confirmationPrompt]);

  // Let's also track whenever the ref becomes available
  const agent = agentRef.current;
  useEffect(() => {
    if (isLoggingEnabled()) {
      log(`agentRef.current is now ${Boolean(agent)}`);
    }
  }, [agent]);

  // ---------------------------------------------------------------------
  // Dynamic layout constraints – keep total rendered rows <= terminal rows
  // ---------------------------------------------------------------------

  const { rows: terminalRows } = useTerminalSize();

  useEffect(() => {
    const processInitialInputItems = async () => {
      if (
        (!initialPrompt || initialPrompt.trim() === "") &&
        (!initialImagePaths || initialImagePaths.length === 0)
      ) {
        return;
      }
      // Ensure agent is ready before processing initial input
      if (!agent) {
        log("Agent not ready yet for initial prompt processing");
        return;
      }
      const inputItems = [
        await createInputItem(initialPrompt || "", initialImagePaths || []),
      ];
      // Clear them to prevent subsequent runs
      setInitialPrompt("");
      setInitialImagePaths([]);
      agent?.run(inputItems);
    };
    // Add a small delay or check agent readiness before processing
    // This ensures the agentRef is populated after the effect that creates it runs.
    const timeoutId = setTimeout(() => {
      processInitialInputItems();
    }, 100); // Adjust delay as needed, or use a more robust readiness check

    return () => clearTimeout(timeoutId);
  }, [agent, initialPrompt, initialImagePaths]); // Keep dependencies

  // ────────────────────────────────────────────────────────────────
  // In-app warning if CLI --model isn't in fetched list
  // ────────────────────────────────────────────────────────────────
  useEffect(() => {
    (async () => {
      const available = await getAvailableModels();
      if (model && available.length > 0 && !available.includes(model)) {
        setItems((prev) => [
          ...prev,
          {
            id: `unknown-model-${Date.now()}`,
            type: "message",
            role: "system",
            status: "completed" as const,
            content: [
              {
                type: "input_text",
                text: `Warning: model "${model}" is not in the list of available models returned by OpenAI.`,
              },
            ],
          },
        ]);
      }
    })();
    // run once on mount
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Just render every item in order, no grouping/collapse
  const lastMessageBatch = items.map((item) => ({ item }));
  const groupCounts: Record<string, number> = {};
  const userMsgCount = items.filter(
    (i) => i.type === "message" && i.role === "user",
  ).length;

  const contextLeftPercent = useMemo(
    () => calculateContextPercentRemaining(items, model),
    [items, model],
  );

  return (
    <Box flexDirection="column">
      <Box flexDirection="column">
        {agent ? (
          <TerminalMessageHistory
            batch={lastMessageBatch}
            groupCounts={groupCounts}
            items={items}
            userMsgCount={userMsgCount}
            confirmationPrompt={confirmationPrompt}
            loading={loading}
            thinkingSeconds={thinkingSeconds}
            fullStdout={fullStdout}
            headerProps={{
              terminalRows,
              version: CLI_VERSION,
              PWD,
              model,
              approvalPolicy,
              colorsByPolicy,
              agent,
              initialImagePaths,
            }}
          />
        ) : (
          <Box>
            <Text color="gray">Initializing agent…</Text>
          </Box>
        )}

        {/* Render tool results */}
        {toolResults.length > 0 && (
          <Box flexDirection="column" marginY={1}>
            {toolResults.map((result, index) => (
              <TerminalChatToolExecutionItem
                key={`tool-result-${index}`}
                toolResult={result}
              />
            ))}
          </Box>
        )}

        {agent && (
          <TerminalChatInput
            loading={loading}
            setItems={setItems}
            isNew={Boolean(items.length === 0)}
            setLastResponseId={setLastResponseId}
            confirmationPrompt={confirmationPrompt}
            submitConfirmation={(
              decision: ReviewDecision,
              customDenyMessage?: string,
            ) =>
              submitConfirmation({
                decision,
                customDenyMessage,
              })
            }
            contextLeftPercent={contextLeftPercent}
            openOverlay={() => setOverlayMode("history")}
            openModelOverlay={() => setOverlayMode("model")}
            openApprovalOverlay={() => setOverlayMode("approval")}
            openHelpOverlay={() => setOverlayMode("help")}
            active={overlayMode === "none"}
            interruptAgent={() => {
              if (!agent) {
                return;
              }
              if (isLoggingEnabled()) {
                log(
                  "TerminalChat: interruptAgent invoked – calling agent.cancel()",
                );
              }
              agent.cancel();
              setLoading(false);
            }}
            submitInput={(inputs) => {
              agent.run(inputs, lastResponseId || "");
              return {};
            }}
          />
        )}
        {overlayMode === "history" && (
          <HistoryOverlay items={items} onExit={() => setOverlayMode("none")} />
        )}
        {overlayMode === "model" && (
          <ModelOverlay
            currentModel={model}
            hasLastResponse={Boolean(lastResponseId)}
            onSelect={(newModel) => {
              if (isLoggingEnabled()) {
                log(
                  "TerminalChat: interruptAgent invoked – calling agent.cancel()",
                );
                if (!agent) {
                  log("TerminalChat: agent is not ready yet");
                }
              }
              agent?.cancel();
              setLoading(false);

              setModel(newModel);
              setLastResponseId((prev) =>
                prev && newModel !== model ? null : prev,
              );

              setItems((prev) => [
                ...prev,
                {
                  id: `switch-model-${Date.now()}`,
                  type: "message",
                  role: "system",
                  status: "completed" as const,
                  content: [
                    {
                      type: "input_text",
                      text: `Switched model to ${newModel}`,
                    },
                  ],
                },
              ]);

              setOverlayMode("none");
            }}
            onExit={() => setOverlayMode("none")}
          />
        )}

        {overlayMode === "approval" && (
          <ApprovalModeOverlay
            currentMode={approvalPolicy}
            onSelect={(newMode) => {
              agent?.cancel();
              setLoading(false);
              if (newMode === approvalPolicy) {
                return;
              }
              setApprovalPolicy(newMode as ApprovalPolicy);
              setItems((prev) => [
                ...prev,
                {
                  id: `switch-approval-${Date.now()}`,
                  type: "message",
                  role: "system",
                  status: "completed" as const,
                  content: [
                    {
                      type: "input_text",
                      text: `Switched approval mode to ${newMode}`,
                    },
                  ],
                },
              ]);

              setOverlayMode("none");
            }}
            onExit={() => setOverlayMode("none")}
          />
        )}

        {overlayMode === "help" && (
          <HelpOverlay onExit={() => setOverlayMode("none")} />
        )}
      </Box>
    </Box>
  );
}
