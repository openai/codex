import type { MultilineTextEditorHandle } from "./multiline-editor";
import type { ReviewDecision } from "../../utils/agent/review.js";
import type { FileSystemSuggestion } from "../../utils/file-system-suggestions.js";
import type { HistoryEntry } from "../../utils/storage/command-history.js";
import type {
  ResponseInputItem,
  ResponseItem,
} from "openai/resources/responses/responses.mjs";

import MultilineTextEditor from "./multiline-editor";
import { TerminalChatCommandReview } from "./terminal-chat-command-review.js";
import TextCompletions from "./terminal-chat-completions.js";
import { loadConfig } from "../../utils/config.js";
import { getFileSystemSuggestions } from "../../utils/file-system-suggestions.js";
import { expandFileTags } from "../../utils/file-tag-utils";
import { createInputItem } from "../../utils/input-utils.js";
import { log } from "../../utils/logger/log.js";
import { setSessionId } from "../../utils/session.js";
import { SLASH_COMMANDS, type SlashCommand } from "../../utils/slash-commands";
import {
  loadCommandHistory,
  addToHistory,
} from "../../utils/storage/command-history.js";
import { clearTerminal, onExit } from "../../utils/terminal.js";
import { Box, Text, useApp, useInput, useStdin } from "ink";
import { fileURLToPath } from "node:url";
import React, {
  useCallback,
  useState,
  Fragment,
  useEffect,
  useRef,
} from "react";
import { useInterval } from "use-interval";

const suggestions = [
  "explain this codebase to me",
  "fix any build errors",
  "are there any bugs in my code?",
];

export default function TerminalChatInput({
  isNew,
  loading,
  submitInput,
  confirmationPrompt,
  explanation,
  submitConfirmation,
  setLastResponseId,
  setItems,
  contextLeftPercent,
  openOverlay,
  openModelOverlay,
  openApprovalOverlay,
  openHelpOverlay,
  openDiffOverlay,
  openSessionsOverlay,
  onCompact,
  interruptAgent,
  active,
  thinkingSeconds,
  items = [],
}: {
  isNew: boolean;
  loading: boolean;
  submitInput: (input: Array<ResponseInputItem>) => void;
  confirmationPrompt: React.ReactNode | null;
  explanation?: string;
  submitConfirmation: (
    decision: ReviewDecision,
    customDenyMessage?: string,
  ) => void;
  setLastResponseId: (lastResponseId: string) => void;
  setItems: React.Dispatch<React.SetStateAction<Array<ResponseItem>>>;
  contextLeftPercent: number;
  openOverlay: () => void;
  openModelOverlay: () => void;
  openApprovalOverlay: () => void;
  openHelpOverlay: () => void;
  openDiffOverlay: () => void;
  openSessionsOverlay: () => void;
  onCompact: () => void;
  interruptAgent: () => void;
  active: boolean;
  thinkingSeconds: number;
  // New: current conversation items so we can include them in bug reports
  items?: Array<ResponseItem>;
}): React.ReactElement {
  // Slash command suggestion index
  const [selectedSlashSuggestion, setSelectedSlashSuggestion] =
    useState<number>(0);
  const app = useApp();
  const [selectedSuggestion, setSelectedSuggestion] = useState<number>(0);
  const [input, setInput] = useState("");
  const [history, setHistory] = useState<Array<HistoryEntry>>([]);
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [draftInput, setDraftInput] = useState<string>("");
  const [skipNextSubmit, setSkipNextSubmit] = useState<boolean>(false);
  const [fsSuggestions, setFsSuggestions] = useState<
    Array<FileSystemSuggestion>
  >([]);
  const [selectedCompletion, setSelectedCompletion] = useState<number>(-1);
  // Multiline text editor key to force remount after submission
  const [editorState, setEditorState] = useState<{
    key: number;
    initialCursorOffset?: number;
  }>({ key: 0 });
  // Imperative handle from the multiline editor so we can query caret position
  const editorRef = useRef<MultilineTextEditorHandle | null>(null);
  // Track the caret row across keystrokes
  const prevCursorRow = useRef<number | null>(null);
  const prevCursorWasAtLastRow = useRef<boolean>(false);

  // --- Helper for updating input, remounting editor, and moving cursor to end ---
  const applyFsSuggestion = useCallback((newInputText: string) => {
    setInput(newInputText);
    setEditorState((s) => ({
      key: s.key + 1,
      initialCursorOffset: newInputText.length,
    }));
  }, []);

  // --- Helper for updating file system suggestions ---
  function updateFsSuggestions(
    txt: string,
    alwaysUpdateSelection: boolean = false,
  ) {
    // Clear file system completions if a space is typed
    if (txt.endsWith(" ")) {
      setFsSuggestions([]);
      setSelectedCompletion(-1);
    } else {
      // Determine the current token (last whitespace-separated word)
      const words = txt.trim().split(/\s+/);
      const lastWord = words[words.length - 1] ?? "";

      const shouldUpdateSelection =
        lastWord.startsWith("@") || alwaysUpdateSelection;

      // Strip optional leading '@' for the path prefix
      let pathPrefix: string;
      if (lastWord.startsWith("@")) {
        pathPrefix = lastWord.slice(1);
        // If only '@' is typed, list everything in the current directory
        pathPrefix = pathPrefix.length === 0 ? "./" : pathPrefix;
      } else {
        pathPrefix = lastWord;
      }

      if (shouldUpdateSelection) {
        const completions = getFileSystemSuggestions(pathPrefix);
        setFsSuggestions(completions);
        if (completions.length > 0) {
          setSelectedCompletion((prev) =>
            prev < 0 || prev >= completions.length ? 0 : prev,
          );
        } else {
          setSelectedCompletion(-1);
        }
      } else if (fsSuggestions.length > 0) {
        // Token cleared → clear menu
        setFsSuggestions([]);
        setSelectedCompletion(-1);
      }
    }
  }

  /**
   * Result of replacing text with a file system suggestion
   */
  interface ReplacementResult {
    /** The new text with the suggestion applied */
    text: string;
    /** The selected suggestion if a replacement was made */
    suggestion: FileSystemSuggestion | null;
    /** Whether a replacement was actually made */
    wasReplaced: boolean;
  }

  // --- Helper for replacing input with file system suggestion ---
  function getFileSystemSuggestion(
    txt: string,
    requireAtPrefix: boolean = false,
  ): ReplacementResult {
    if (fsSuggestions.length === 0 || selectedCompletion < 0) {
      return { text: txt, suggestion: null, wasReplaced: false };
    }

    const words = txt.trim().split(/\s+/);
    const lastWord = words[words.length - 1] ?? "";

    // Check if @ prefix is required and the last word doesn't have it
    if (requireAtPrefix && !lastWord.startsWith("@")) {
      return { text: txt, suggestion: null, wasReplaced: false };
    }

    const selected = fsSuggestions[selectedCompletion];
    if (!selected) {
      return { text: txt, suggestion: null, wasReplaced: false };
    }

    const replacement = lastWord.startsWith("@")
      ? `@${selected.path}`
      : selected.path;
    words[words.length - 1] = replacement;
    return {
      text: words.join(" "),
      suggestion: selected,
      wasReplaced: true,
    };
  }

  // Load command history on component mount
  useEffect(() => {
    async function loadHistory() {
      const historyEntries = await loadCommandHistory();
      setHistory(historyEntries);
    }

    loadHistory();
  }, []);
  // Reset slash suggestion index when input prefix changes
  useEffect(() => {
    if (input.trim().startsWith("/")) {
      setSelectedSlashSuggestion(0);
    }
  }, [input]);

  useInput(
    (_input, _key) => {
      // Slash command navigation: up/down to select, enter to fill
      if (!confirmationPrompt && !loading && input.trim().startsWith("/")) {
        const prefix = input.trim();
        const matches = SLASH_COMMANDS.filter((cmd: SlashCommand) =>
          cmd.command.startsWith(prefix),
        );
        if (matches.length > 0) {
          if (_key.tab) {
            // Cycle and fill slash command suggestions on Tab
            const len = matches.length;
            // Determine new index based on shift state
            const nextIdx = _key.shift
              ? selectedSlashSuggestion <= 0
                ? len - 1
                : selectedSlashSuggestion - 1
              : selectedSlashSuggestion >= len - 1
                ? 0
                : selectedSlashSuggestion + 1;
            setSelectedSlashSuggestion(nextIdx);
            // Autocomplete the command in the input
            const match = matches[nextIdx];
            if (!match) {
              return;
            }
            const cmd = match.command;
            setInput(cmd);
            setDraftInput(cmd);
            return;
          }
          if (_key.upArrow) {
            setSelectedSlashSuggestion((prev) =>
              prev <= 0 ? matches.length - 1 : prev - 1,
            );
            return;
          }
          if (_key.downArrow) {
            setSelectedSlashSuggestion((prev) =>
              prev < 0 || prev >= matches.length - 1 ? 0 : prev + 1,
            );
            return;
          }
          if (_key.return) {
            // Execute the currently selected slash command
            const selIdx = selectedSlashSuggestion;
            const cmdObj = matches[selIdx];
            if (cmdObj) {
              const cmd = cmdObj.command;
              setInput("");
              setDraftInput("");
              setSelectedSlashSuggestion(0);
              switch (cmd) {
                case "/history":
                  openOverlay();
                  break;
                case "/sessions":
                  openSessionsOverlay();
                  break;
                case "/help":
                  openHelpOverlay();
                  break;
                case "/compact":
                  onCompact();
                  break;
                case "/model":
                  openModelOverlay();
                  break;
                case "/approval":
                  openApprovalOverlay();
                  break;
                case "/diff":
                  openDiffOverlay();
                  break;
                case "/bug":
                  onSubmit(cmd);
                  break;
                case "/clear":
                  onSubmit(cmd);
                  break;
                case "/clearhistory":
                  onSubmit(cmd);
                  break;
                default:
                  break;
              }
            }
            return;
          }
        }
      }
      if (!confirmationPrompt && !loading) {
        if (fsSuggestions.length > 0) {
          if (_key.upArrow) {
            setSelectedCompletion((prev) =>
              prev <= 0 ? fsSuggestions.length - 1 : prev - 1,
            );
            return;
          }

          if (_key.downArrow) {
            setSelectedCompletion((prev) =>
              prev >= fsSuggestions.length - 1 ? 0 : prev + 1,
            );
            return;
          }

          if (_key.tab && selectedCompletion >= 0) {
            const { text: newText, wasReplaced } =
              getFileSystemSuggestion(input);

            // Only proceed if the text was actually changed
            if (wasReplaced) {
              applyFsSuggestion(newText);
              setFsSuggestions([]);
              setSelectedCompletion(-1);
            }
            return;
          }
        }

        if (_key.upArrow) {
          let moveThroughHistory = true;

          // Only use history when the caret was *already* on the very first
          // row *before* this key-press.
          const cursorRow = editorRef.current?.getRow?.() ?? 0;
          const cursorCol = editorRef.current?.getCol?.() ?? 0;
          const wasAtFirstRow = (prevCursorRow.current ?? cursorRow) === 0;
          if (!(cursorRow === 0 && wasAtFirstRow)) {
            moveThroughHistory = false;
          }

          // If we are not yet in history mode, then also require that the col is zero so that
          // we only trigger history navigation when the user is at the start of the input.
          if (historyIndex == null && !(cursorRow === 0 && cursorCol === 0)) {
            moveThroughHistory = false;
          }

          // Move through history.
          if (history.length && moveThroughHistory) {
            let newIndex: number;
            if (historyIndex == null) {
              const currentDraft = editorRef.current?.getText?.() ?? input;
              setDraftInput(currentDraft);
              newIndex = history.length - 1;
            } else {
              newIndex = Math.max(0, historyIndex - 1);
            }
            setHistoryIndex(newIndex);

            setInput(history[newIndex]?.command ?? "");
            // Re-mount the editor so it picks up the new initialText
            setEditorState((s) => ({ key: s.key + 1 }));
            return; // handled
          }

          // Otherwise let it propagate.
        }

        if (_key.downArrow) {
          // Only move forward in history when we're already *in* history mode
          // AND the caret sits on the last line of the buffer.
          const wasAtLastRow =
            prevCursorWasAtLastRow.current ??
            editorRef.current?.isCursorAtLastRow() ??
            true;
          if (historyIndex != null && wasAtLastRow) {
            const newIndex = historyIndex + 1;
            if (newIndex >= history.length) {
              setHistoryIndex(null);
              setInput(draftInput);
              setEditorState((s) => ({ key: s.key + 1 }));
            } else {
              setHistoryIndex(newIndex);
              setInput(history[newIndex]?.command ?? "");
              setEditorState((s) => ({ key: s.key + 1 }));
            }
            return; // handled
          }
          // Otherwise let it propagate
        }

        // Defer filesystem suggestion logic to onSubmit if enter key is pressed
        if (!_key.return) {
          // Pressing tab should trigger the file system suggestions
          const shouldUpdateSelection = _key.tab;
          const targetInput = _key.delete ? input.slice(0, -1) : input + _input;
          updateFsSuggestions(targetInput, shouldUpdateSelection);
        }
      }

      // Update the cached cursor position *after* **all** handlers (including
      // the internal <MultilineTextEditor>) have processed this key event.
      //
      // Ink invokes `useInput` callbacks starting with **parent** components
      // first, followed by their descendants. As a result the call above
      // executes *before* the editor has had a chance to react to the key
      // press and update its internal caret position.  When navigating
      // through a multi-line draft with the ↑ / ↓ arrow keys this meant we
      // recorded the *old* cursor row instead of the one that results *after*
      // the key press.  Consequently, a subsequent ↑ still saw
      // `prevCursorRow = 1` even though the caret was already on row 0 and
      // history-navigation never kicked in.
      //
      // Defer the sampling by one tick so we read the *final* caret position
      // for this frame.
      setTimeout(() => {
        prevCursorRow.current = editorRef.current?.getRow?.() ?? null;
        prevCursorWasAtLastRow.current =
          editorRef.current?.isCursorAtLastRow?.() ?? true;
      }, 1);

      if (input.trim() === "" && isNew) {
        if (_key.tab) {
          setSelectedSuggestion(
            (s) => (s + (_key.shift ? -1 : 1)) % (suggestions.length + 1),
          );
        } else if (selectedSuggestion && _key.return) {
          const suggestion = suggestions[selectedSuggestion - 1] || "";
          setInput("");
          setSelectedSuggestion(0);
          submitInput([
            {
              role: "user",
              content: [{ type: "input_text", text: suggestion }],
              type: "message",
            },
          ]);
        }
      } else if (_input === "\u0003" || (_input === "c" && _key.ctrl)) {
        setTimeout(() => {
          app.exit();
          onExit();
          process.exit(0);
        }, 60);
      }
    },
    { isActive: active },
  );

  const onSubmit = useCallback(
    async (value: string) => {
      const inputValue = value.trim();

      // If the user only entered a slash, do not send a chat message.
      if (inputValue === "/") {
        setInput("");
        return;
      }

      // Skip this submit if we just autocompleted a slash command.
      if (skipNextSubmit) {
        setSkipNextSubmit(false);
        return;
      }

      if (!inputValue) {
        return;
      } else if (inputValue === "/history") {
        setInput("");
        openOverlay();
        return;
      } else if (inputValue === "/sessions") {
        setInput("");
        openSessionsOverlay();
        return;
      } else if (inputValue === "/help") {
        setInput("");
        openHelpOverlay();
        return;
      } else if (inputValue === "/diff") {
        setInput("");
        openDiffOverlay();
        return;
      } else if (inputValue === "/compact") {
        setInput("");
        onCompact();
        return;
      } else if (inputValue.startsWith("/model")) {
        setInput("");
        openModelOverlay();
        return;
      } else if (inputValue.startsWith("/approval")) {
        setInput("");
        openApprovalOverlay();
        return;
      } else if (["exit", "q", ":q"].includes(inputValue)) {
        setInput("");
        setTimeout(() => {
          app.exit();
          onExit();
          process.exit(0);
        }, 60); // Wait one frame.
        return;
      } else if (inputValue === "/clear" || inputValue === "clear") {
        setInput("");
        setSessionId("");
        setLastResponseId("");

        // Clear the terminal screen (including scrollback) before resetting context.
        clearTerminal();

        // Emit a system message to confirm the clear action.  We *append*
        // it so Ink's <Static> treats it as new output and actually renders it.
        setItems((prev) => {
          const filteredOldItems = prev.filter((item) => {
            // Remove any token‑heavy entries (user/assistant turns and function calls)
            if (
              item.type === "message" &&
              (item.role === "user" || item.role === "assistant")
            ) {
              return false;
            }
            if (
              item.type === "function_call" ||
              item.type === "function_call_output"
            ) {
              return false;
            }
            return true; // keep developer/system and other meta entries
          });

          return [
            ...filteredOldItems,
            {
              id: `clear-${Date.now()}`,
              type: "message",
              role: "system",
              content: [{ type: "input_text", text: "Terminal cleared" }],
            },
          ];
        });

        return;
      } else if (inputValue === "/clearhistory") {
        setInput("");

        // Import clearCommandHistory function to avoid circular dependencies
        // Using dynamic import to lazy-load the function
        import("../../utils/storage/command-history.js").then(
          async ({ clearCommandHistory }) => {
            await clearCommandHistory();
            setHistory([]);

            // Emit a system message to confirm the history clear action.
            setItems((prev) => [
              ...prev,
              {
                id: `clearhistory-${Date.now()}`,
                type: "message",
                role: "system",
                content: [
                  { type: "input_text", text: "Command history cleared" },
                ],
              },
            ]);
          },
        );

        return;
      } else if (inputValue === "/bug") {
        // Generate a GitHub bug report URL pre‑filled with session details.
        setInput("");

        try {
          const os = await import("node:os");
          const { CLI_VERSION } = await import("../../version.js");
          const { buildBugReportUrl } = await import(
            "../../utils/bug-report.js"
          );

          const url = buildBugReportUrl({
            items: items ?? [],
            cliVersion: CLI_VERSION,
            model: loadConfig().model ?? "unknown",
            platform: [os.platform(), os.arch(), os.release()]
              .map((s) => `\`${s}\``)
              .join(" | "),
          });

          setItems((prev) => [
            ...prev,
            {
              id: `bugreport-${Date.now()}`,
              type: "message",
              role: "system",
              content: [
                {
                  type: "input_text",
                  text: `🔗 Bug report URL: ${url}`,
                },
              ],
            },
          ]);
        } catch (error) {
          // If anything went wrong, notify the user.
          setItems((prev) => [
            ...prev,
            {
              id: `bugreport-error-${Date.now()}`,
              type: "message",
              role: "system",
              content: [
                {
                  type: "input_text",
                  text: `⚠️ Failed to create bug report URL: ${error}`,
                },
              ],
            },
          ]);
        }

        return;
      } else if (inputValue.startsWith("/")) {
        // Handle invalid/unrecognized commands. Only single-word inputs starting with '/'
        // (e.g., /command) that are not recognized are caught here. Any other input, including
        // those starting with '/' but containing spaces (e.g., "/command arg"), will fall through
        // and be treated as a regular prompt.
        const trimmed = inputValue.trim();

        if (/^\/\S+$/.test(trimmed)) {
          setInput("");
          setItems((prev) => [
            ...prev,
            {
              id: `invalidcommand-${Date.now()}`,
              type: "message",
              role: "system",
              content: [
                {
                  type: "input_text",
                  text: `Invalid command "${trimmed}". Use /help to retrieve the list of commands.`,
                },
              ],
            },
          ]);

          return;
        }
      }

      // detect image file paths for dynamic inclusion
      const images: Array<string> = [];
      let text = inputValue;

      // markdown-style image syntax: ![alt](path)
      text = text.replace(/!\[[^\]]*?\]\(([^)]+)\)/g, (_m, p1: string) => {
        images.push(p1.startsWith("file://") ? fileURLToPath(p1) : p1);
        return "";
      });

      // quoted file paths ending with common image extensions (e.g. '/path/to/img.png')
      text = text.replace(
        /['"]([^'"]+?\.(?:png|jpe?g|gif|bmp|webp|svg))['"]/gi,
        (_m, p1: string) => {
          images.push(p1.startsWith("file://") ? fileURLToPath(p1) : p1);
          return "";
        },
      );

      // bare file paths ending with common image extensions
      text = text.replace(
        // eslint-disable-next-line no-useless-escape
        /\b(?:\.[\/\\]|[\/\\]|[A-Za-z]:[\/\\])?[\w-]+(?:[\/\\][\w-]+)*\.(?:png|jpe?g|gif|bmp|webp|svg)\b/gi,
        (match: string) => {
          images.push(
            match.startsWith("file://") ? fileURLToPath(match) : match,
          );
          return "";
        },
      );
      text = text.trim();

      // Expand @file tokens into XML blocks for the model
      const expandedText = await expandFileTags(text);

      const inputItem = await createInputItem(expandedText, images);
      submitInput([inputItem]);

      // Get config for history persistence.
      const config = loadConfig();

      // Add to history and update state.
      const updatedHistory = await addToHistory(value, history, {
        maxSize: config.history?.maxSize ?? 1000,
        saveHistory: config.history?.saveHistory ?? true,
        sensitivePatterns: config.history?.sensitivePatterns ?? [],
      });

      setHistory(updatedHistory);
      setHistoryIndex(null);
      setDraftInput("");
      setSelectedSuggestion(0);
      setInput("");
      setFsSuggestions([]);
      setSelectedCompletion(-1);
    },
    [
      setInput,
      submitInput,
      setLastResponseId,
      setItems,
      app,
      setHistory,
      setHistoryIndex,
      openOverlay,
      openApprovalOverlay,
      openModelOverlay,
      openHelpOverlay,
      openDiffOverlay,
      openSessionsOverlay,
      history,
      onCompact,
      skipNextSubmit,
      items,
    ],
  );

  if (confirmationPrompt) {
    return (
      <TerminalChatCommandReview
        confirmationPrompt={confirmationPrompt}
        onReviewCommand={submitConfirmation}
        // allow switching approval mode via 'v'
        onSwitchApprovalMode={openApprovalOverlay}
        explanation={explanation}
        // disable when input is inactive (e.g., overlay open)
        isActive={active}
      />
    );
  }

  return (
    <Box flexDirection="column">
      <Box borderStyle="round">
        {loading ? (
          <TerminalChatInputThinking
            onInterrupt={interruptAgent}
            active={active}
            thinkingSeconds={thinkingSeconds}
          />
        ) : (
          <Box paddingX={1}>
            <MultilineTextEditor
              ref={editorRef}
              onChange={(txt: string) => {
                setDraftInput(txt);
                if (historyIndex != null) {
                  setHistoryIndex(null);
                }
                setInput(txt);
              }}
              key={editorState.key}
              initialCursorOffset={editorState.initialCursorOffset}
              initialText={input}
              height={6}
              focus={active}
              onSubmit={(txt) => {
                // If final token is an @path, replace with filesystem suggestion if available
                const {
                  text: replacedText,
                  suggestion,
                  wasReplaced,
                } = getFileSystemSuggestion(txt, true);

                // If we replaced @path token with a directory, don't submit
                if (wasReplaced && suggestion?.isDirectory) {
                  applyFsSuggestion(replacedText);
                  // Update suggestions for the new directory
                  updateFsSuggestions(replacedText, true);
                  return;
                }

                onSubmit(replacedText);
                setEditorState((s) => ({ key: s.key + 1 }));
                setInput("");
                setHistoryIndex(null);
                setDraftInput("");
              }}
            />
          </Box>
        )}
      </Box>
      {/* Slash command autocomplete suggestions */}
      {input.trim().startsWith("/") && (
        <Box flexDirection="column" paddingX={2} marginBottom={1}>
          {SLASH_COMMANDS.filter((cmd: SlashCommand) =>
            cmd.command.startsWith(input.trim()),
          ).map((cmd: SlashCommand, idx: number) => (
            <Box key={cmd.command}>
              <Text
                backgroundColor={
                  idx === selectedSlashSuggestion ? "blackBright" : undefined
                }
              >
                <Text color="blueBright">{cmd.command}</Text>
                <Text> {cmd.description}</Text>
              </Text>
            </Box>
          ))}
        </Box>
      )}
      <Box paddingX={2} marginBottom={1}>
        {isNew && !input ? (
          <Text dimColor>
            try:{" "}
            {suggestions.map((m, key) => (
              <Fragment key={key}>
                {key !== 0 ? " | " : ""}
                <Text
                  backgroundColor={
                    key + 1 === selectedSuggestion ? "blackBright" : ""
                  }
                >
                  {m}
                </Text>
              </Fragment>
            ))}
          </Text>
        ) : fsSuggestions.length > 0 ? (
          <TextCompletions
            completions={fsSuggestions.map((suggestion) => suggestion.path)}
            selectedCompletion={selectedCompletion}
            displayLimit={5}
          />
        ) : (
          <Text dimColor>
            ctrl+c to exit | "/" to see commands | enter to send
            {contextLeftPercent > 25 && (
              <>
                {" — "}
                <Text color={contextLeftPercent > 40 ? "green" : "yellow"}>
                  {Math.round(contextLeftPercent)}% context left
                </Text>
              </>
            )}
            {contextLeftPercent <= 25 && (
              <>
                {" — "}
                <Text color="red">
                  {Math.round(contextLeftPercent)}% context left — send
                  "/compact" to condense context
                </Text>
              </>
            )}
          </Text>
        )}
      </Box>
    </Box>
  );
}

function TerminalChatInputThinking({
  onInterrupt,
  active,
  thinkingSeconds,
}: {
  onInterrupt: () => void;
  active: boolean;
  thinkingSeconds: number;
}) {
  const [awaitingConfirm, setAwaitingConfirm] = useState(false);
  const [dots, setDots] = useState("");

  // Animate ellipsis
  useInterval(() => {
    setDots((prev) => (prev.length < 3 ? prev + "." : ""));
  }, 500);

  // Spinner frames with embedded seconds
  const ballFrames = [
    "( ●    )",
    "(  ●   )",
    "(   ●  )",
    "(    ● )",
    "(     ●)",
    "(    ● )",
    "(   ●  )",
    "(  ●   )",
    "( ●    )",
    "(●     )",
  ];
  const [frame, setFrame] = useState(0);

  useInterval(() => {
    setFrame((idx) => (idx + 1) % ballFrames.length);
  }, 80);

  // Keep the elapsed‑seconds text fixed while the ball animation moves.
  const frameTemplate = ballFrames[frame] ?? ballFrames[0];
  const frameWithSeconds = `${frameTemplate} ${thinkingSeconds}s`;

  // ---------------------------------------------------------------------
  // Raw stdin listener to catch the case where the terminal delivers two
  // consecutive ESC bytes ("\x1B\x1B") in a *single* chunk. Ink's `useInput`
  // collapses that sequence into one key event, so the regular two‑step
  // handler above never sees the second press.  By inspecting the raw data
  // we can identify this special case and trigger the interrupt while still
  // requiring a double press for the normal single‑byte ESC events.
  // ---------------------------------------------------------------------

  const { stdin, setRawMode } = useStdin();

  React.useEffect(() => {
    if (!active) {
      return;
    }

    // Ensure raw mode – already enabled by Ink when the component has focus,
    // but called defensively in case that assumption ever changes.
    setRawMode?.(true);

    const onData = (data: Buffer | string) => {
      if (awaitingConfirm) {
        return; // already awaiting a second explicit press
      }

      // Handle both Buffer and string forms.
      const str = Buffer.isBuffer(data) ? data.toString("utf8") : data;
      if (str === "\x1b\x1b") {
        // Treat as the first Escape press – prompt the user for confirmation.
        log(
          "raw stdin: received collapsed ESC ESC – starting confirmation timer",
        );
        setAwaitingConfirm(true);
        setTimeout(() => setAwaitingConfirm(false), 1500);
      }
    };

    stdin?.on("data", onData);

    return () => {
      stdin?.off("data", onData);
    };
  }, [stdin, awaitingConfirm, onInterrupt, active, setRawMode]);

  // No local timer: the parent component supplies the elapsed time via props.

  // Listen for the escape key to allow the user to interrupt the current
  // operation. We require two presses within a short window (1.5s) to avoid
  // accidental cancellations.
  useInput(
    (_input, key) => {
      if (!key.escape) {
        return;
      }

      if (awaitingConfirm) {
        log("useInput: second ESC detected – triggering onInterrupt()");
        onInterrupt();
        setAwaitingConfirm(false);
      } else {
        log("useInput: first ESC detected – waiting for confirmation");
        setAwaitingConfirm(true);
        setTimeout(() => setAwaitingConfirm(false), 1500);
      }
    },
    { isActive: active },
  );

  return (
    <Box width="100%" flexDirection="column" gap={1}>
      <Box
        flexDirection="row"
        width="100%"
        justifyContent="space-between"
        paddingRight={1}
      >
        <Box gap={2}>
          <Text>{frameWithSeconds}</Text>
          <Text>
            Thinking
            {dots}
          </Text>
        </Box>
        <Text>
          <Text dimColor>press</Text> <Text bold>Esc</Text>{" "}
          {awaitingConfirm ? (
            <Text bold>again</Text>
          ) : (
            <Text dimColor>twice</Text>
          )}{" "}
          <Text dimColor>to interrupt</Text>
        </Text>
      </Box>
    </Box>
  );
}
