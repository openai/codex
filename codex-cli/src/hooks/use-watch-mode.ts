import type { AgentLoop } from "../utils/agent/agent-loop";
import type { ResponseItem } from "openai/resources/responses/responses";

import { createInputItem } from "../utils/input-utils";
import { isLoggingEnabled, log } from "../utils/logger/log";
import { 
  findAllTriggers, 
  extractContextAroundTrigger 
} from "../utils/watch-mode-utils";
import chokidar from "chokidar";
import fs from "fs";
import ignore from "ignore";
import path from "path";
import { useEffect } from "react";

/**
 * A custom hook that sets up file watching for AI trigger comments
 */
export function useWatchMode({
  enabled,
  agent,
  lastResponseId,
  setItems,
}: {
  enabled: boolean;
  agent: AgentLoop | undefined;
  lastResponseId: string | null;
  setItems: React.Dispatch<React.SetStateAction<Array<ResponseItem>>>;
}): void {
  useEffect(() => {
    // If watch mode is not enabled, do nothing
    if (!enabled) {
      return;
    }

    // Store matches of AI triggers for each file
    const fileAITriggerMatches: Map<
      string,
      Array<RegExpMatchArray>
    > = new Map();

    // Function to register file content matches when a file is added
    const registerFileMatches = (filePath: string) => {
      try {
        const content = fs.readFileSync(filePath, "utf-8");
        const matches = findAllTriggers(content);
        fileAITriggerMatches.set(filePath, matches);

        // Only log if we found matches
        if (matches.length > 0) {
          log(
            `Watch mode: File registered with ${matches.length} AI triggers: ${filePath}`,
          );
        }
      } catch (error) {
        log(`Watch mode error registering file ${filePath}: ${error}`);
      }
    };

    // Load .gitignore and create ignore matcher
    let ig = ignore();
    try {
      const gitignorePath = path.resolve(process.cwd(), ".gitignore");
      const gitignore = fs.readFileSync(gitignorePath, "utf8");
      ig = ig.add(gitignore);
    } catch (err) {
      log("No .gitignore found, proceeding without ignore rules.");
    }

    // Custom function to check if path should be ignored
    const shouldIgnore = (filepath: string) => {
      if (!filepath) {
        return false;
      } // skip empty strings
      const relPath = path.relative(process.cwd(), filepath);
      if (!relPath) {
        return false;
      } // skip paths that resolve to ''
      return ig.ignores(relPath);
    };

    // Create file watcher
    const watcher = chokidar.watch(".", {
      persistent: true,
      ignoreInitial: true,
      ignored: shouldIgnore,
    });

    // Set up handler for new files
    watcher.on("add", registerFileMatches);

    // Handle file removal
    watcher.on("unlink", (filePath: string) => {
      fileAITriggerMatches.delete(filePath);
      log(`Watch mode: File removed from tracking: ${filePath}`);
    });

    // Set up change event handler
    watcher.on("change", async (filePath: string) => {
      try {
        if (isLoggingEnabled()) {
          log(`Watch mode: File changed: ${filePath}`);
        }

        // Get the current content and find all AI triggers
        const currentContent = fs.readFileSync(filePath, "utf-8");
        const currentMatches = findAllTriggers(currentContent);

        // Get previous matches (if any)
        const previousMatches = fileAITriggerMatches.get(filePath) || [];

        // Update our tracking map with current matches
        fileAITriggerMatches.set(filePath, currentMatches);

        // Find new matches that weren't in the previous version
        const newMatches = currentMatches.filter((currentMatch) => {
          // Check if this trigger text existed in any of the previous matches
          return !previousMatches.some((prevMatch) => {
            return (
              prevMatch[0] === currentMatch[0] &&
              prevMatch.index === currentMatch.index
            );
          });
        });

        if (newMatches.length === 0) {
          return;
        }

        if (isLoggingEnabled()) {
          log(
            `Watch mode: Found ${newMatches.length} new AI triggers in ${filePath}`,
          );
        }

        // Process each new match
        await Promise.all(
          newMatches.map(async (newMatch) => {
            const relativePath = path.relative(process.cwd(), filePath);

            // Extract the context and instruction around the trigger
            const { context, instruction } = extractContextAroundTrigger(
              currentContent,
              newMatch,
            );

            // Add a message to show the trigger detection
            setItems((prev) => [
              ...prev,
              {
                id: `trigger-detected-${Date.now()}`,
                type: "message",
                role: "system",
                content: [
                  {
                    type: "input_text",
                    text:
                      `📝 Trigger detected in ${filePath}\n` +
                      `📋 Instruction: ${instruction}\n` +
                      `⚙️ Processing...`,
                  },
                ],
              } as ResponseItem,
            ]);

            // Build the prompt with file information, context, and the specific instruction
            const fileExt = path.extname(filePath);

            const prompt = `I found a comment with an instruction to: "${instruction}" in the file ${relativePath}.

Here's the relevant section of code:

\`\`\`${fileExt}
${context}
\`\`\`

Please address the specific request: "${instruction}". Analyze the code and provide an appropriate solution based on this instruction.`;

            // Create input item and run the agent if available
            if (agent) {
              const inputItem = await createInputItem(prompt, []);
              agent.run([inputItem], lastResponseId || "");
            }
          }),
        );
      } catch (error) {
        log(`Watch mode error processing file ${filePath}: ${error}`);
      }
    });

    // Log that the watcher is ready
    watcher.on("ready", () => {
      log("Watch mode: Ready and monitoring files for changes");
    });

    // Cleanup watcher when component unmounts
    return () => {
      watcher.close().catch((err) => log(`Error closing watcher: ${err}`));
    };
  }, [enabled, agent, lastResponseId, setItems]);
}
