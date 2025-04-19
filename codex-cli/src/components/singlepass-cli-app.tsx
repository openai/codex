/* eslint-disable no-await-in-loop */

import type { AppConfig } from "../utils/config";
import type { FileOperation } from "../utils/singlepass/file_ops";

import Spinner from "./vendor/ink-spinner"; // Third‑party / vendor components
import TextInput from "./vendor/ink-text-input";
import { OPENAI_TIMEOUT_MS, OPENAI_BASE_URL } from "../utils/config";
import {
  generateDiffSummary,
  generateEditSummary,
} from "../utils/singlepass/code_diff";
import { renderTaskContext } from "../utils/singlepass/context";
import {
  getFileContents,
  loadIgnorePatterns,
  makeAsciiDirectoryStructure,
} from "../utils/singlepass/context_files";
import { EditedFilesSchema } from "../utils/singlepass/file_ops";
import * as fsSync from "fs";
import * as fsPromises from "fs/promises";
import { Box, Text, useApp, useInput } from "ink";
import OpenAI from "openai";
import { zodResponseFormat } from "openai/helpers/zod";
import path from "path";
import React, { useEffect, useState, useRef } from "react";

/** Maximum number of characters allowed in the context passed to the model. */
const MAX_CONTEXT_CHARACTER_LIMIT = 2_000_000;

// --- prompt history support (same as for rest of CLI) ---
const PROMPT_HISTORY_KEY = "__codex_singlepass_prompt_history";
function loadPromptHistory(): Array<string> {
  try {
    if (typeof localStorage !== "undefined") {
      const raw = localStorage.getItem(PROMPT_HISTORY_KEY);
      if (raw) {
        return JSON.parse(raw);
      }
    }
  } catch {
    // ignore
  }
  // fallback to process.env-based temp storage if localStorage isn't available
  try {
    if (process && process.env && process.env["HOME"]) {
      const p = path.join(
        process.env["HOME"],
        ".codex_singlepass_history.json",
      );
      if (fsSync.existsSync(p)) {
        return JSON.parse(fsSync.readFileSync(p, "utf8"));
      }
    }
  } catch {
    // ignore
  }
  return [];
}

function savePromptHistory(history: Array<string>) {
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(PROMPT_HISTORY_KEY, JSON.stringify(history));
    }
  } catch {
    // ignore
  }
  // fallback to process.env-based temp storage if localStorage isn't available
  try {
    if (process && process.env && process.env["HOME"]) {
      const p = path.join(
        process.env["HOME"],
        ".codex_singlepass_history.json",
      );
      fsSync.writeFileSync(p, JSON.stringify(history), "utf8");
    }
  } catch {
    // ignore
  }
}

/**
 * Small animated spinner shown while the request to OpenAI is in‑flight.
 */
function WorkingSpinner({ text = "Working" }: { text?: string }) {
  const [dots, setDots] = useState("");

  useEffect(() => {
    const interval = setInterval(() => {
      setDots((d) => (d.length < 3 ? d + "." : ""));
    }, 400);
    return () => clearInterval(interval);
  }, []);

  return (
    <Box gap={2}>
      <Spinner type="ball" />
      <Text>
        {text}
        {dots}
      </Text>
    </Box>
  );
}

function DirectoryInfo({
  rootPath,
  files,
  contextLimit,
  showStruct = false,
}: {
  rootPath: string;
  files: Array<{ path: string; content: string }>;
  contextLimit: number;
  showStruct?: boolean;
}) {
  const asciiStruct = React.useMemo(
    () =>
      showStruct
        ? makeAsciiDirectoryStructure(
            rootPath,
            files.map((fc) => fc.path),
          )
        : null,
    [showStruct, rootPath, files],
  );
  const totalChars = files.reduce((acc, fc) => acc + fc.content.length, 0);

  return (
    <Box flexDirection="column">
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor="gray"
        width={80}
        paddingX={1}
      >
        <Text>
          <Text color="magentaBright">↳</Text> <Text bold>Directory:</Text>{" "}
          {rootPath}
        </Text>
        <Text>
          <Text color="magentaBright">↳</Text>{" "}
          <Text bold>Paths in context:</Text> {rootPath} ({files.length} files)
        </Text>
        <Text>
          <Text color="magentaBright">↳</Text> <Text bold>Context size:</Text>{" "}
          {totalChars} / {contextLimit} ( ~
          {((totalChars / contextLimit) * 100).toFixed(2)}% )
        </Text>
        {showStruct ? (
          <Text>
            <Text color="magentaBright">↳</Text>
            <Text bold>Context structure:</Text>
            <Text>{asciiStruct}</Text>
          </Text>
        ) : (
          <Text>
            <Text color="magentaBright">↳</Text>{" "}
            <Text bold>Context structure:</Text>{" "}
            <Text dimColor>
              Hidden. Type <Text color="cyan">/context</Text> to show it.
            </Text>
          </Text>
        )}
        {totalChars > contextLimit ? (
          <Text color="red">
            Files exceed context limit. See breakdown below.
          </Text>
        ) : null}
      </Box>
    </Box>
  );
}

function SummaryAndDiffs({
  summary,
  diffs,
}: {
  summary: string;
  diffs: string;
}) {
  return (
    <Box flexDirection="column" marginTop={1}>
      <Text color="yellow" bold>
        Summary:
      </Text>
      <Text>{summary}</Text>
      <Text color="cyan" bold>
        Proposed Diffs:
      </Text>
      <Text>{diffs}</Text>
    </Box>
  );
}

/* -------------------------------------------------------------------------- */
/*                               Input prompts                                */
/* -------------------------------------------------------------------------- */

function InputPrompt({
  message,
  onSubmit,
  onCtrlC,
}: {
  message: string;
  onSubmit: (val: string) => void;
  onCtrlC?: () => void;
}) {
  const [value, setValue] = useState("");
  const [history] = useState(() => loadPromptHistory());
  const [historyIndex, setHistoryIndex] = useState<number | null>(null);
  const [draftInput, setDraftInput] = useState<string>("");
  const [, setShowDirInfo] = useState(false);

  useInput((input, key) => {
    if ((key.ctrl && (input === "c" || input === "C")) || input === "\u0003") {
      // Ctrl+C pressed – treat as interrupt
      if (onCtrlC) {
        onCtrlC();
      } else {
        process.exit(0);
      }
    } else if (key.return) {
      if (value.trim() !== "") {
        // Save to history (front of list)
        const updated =
          history[history.length - 1] === value ? history : [...history, value];
        savePromptHistory(updated.slice(-50));
      }
      onSubmit(value.trim());
    } else if (key.upArrow) {
      if (history.length > 0) {
        if (historyIndex == null) {
          setDraftInput(value);
        }
        let newIndex: number;
        if (historyIndex == null) {
          newIndex = history.length - 1;
        } else {
          newIndex = Math.max(0, historyIndex - 1);
        }
        setHistoryIndex(newIndex);
        setValue(history[newIndex] ?? "");
      }
    } else if (key.downArrow) {
      if (historyIndex == null) {
        return;
      }
      const newIndex = historyIndex + 1;
      if (newIndex >= history.length) {
        setHistoryIndex(null);
        setValue(draftInput);
      } else {
        setHistoryIndex(newIndex);
        setValue(history[newIndex] ?? "");
      }
    } else if (input === "/context" || input === ":context") {
      setShowDirInfo(true);
    }
  });

  return (
    <Box flexDirection="column">
      <Box>
        <Text>{message}</Text>
        <TextInput
          value={value}
          onChange={setValue}
          placeholder="Type here…"
          showCursor
          focus
        />
      </Box>
    </Box>
  );
}

function ConfirmationPrompt({
  message,
  onResult,
}: {
  message: string;
  onResult: (accept: boolean) => void;
}) {
  useInput((input, key) => {
    if (key.return || input.toLowerCase() === "y") {
      onResult(true);
    } else if (input.toLowerCase() === "n" || key.escape) {
      onResult(false);
    }
  });

  return (
    <Box gap={1}>
      <Text>{message} [y/N] </Text>
    </Box>
  );
}

function ContinuePrompt({ onResult }: { onResult: (cont: boolean) => void }) {
  useInput((input, key) => {
    if (input.toLowerCase() === "y" || key.return) {
      onResult(true);
    } else if (input.toLowerCase() === "n" || key.escape) {
      onResult(false);
    }
  });

  return (
    <Box gap={1}>
      <Text>Do you want to apply another edit? [y/N] </Text>
    </Box>
  );
}

/* -------------------------------------------------------------------------- */
/*                               Main component                               */
/* -------------------------------------------------------------------------- */

export interface SinglePassAppProps {
  originalPrompt?: string;
  config: AppConfig;
  rootPath: string;
  onExit?: () => void;
}

export function SinglePassApp({
  originalPrompt,
  config,
  rootPath,
  onExit,
}: SinglePassAppProps): JSX.Element {
  const app = useApp();
  const [state, setState] = useState<
    | "init"
    | "prompt"
    | "thinking"
    | "confirm"
    | "skipped"
    | "applied"
    | "noops"
    | "error"
    | "interrupted"
  >("init");

  // we don't need to read the current prompt / spinner state outside of
  // updating functions, so we intentionally ignore the first tuple element.
  const [, setPrompt] = useState(originalPrompt ?? "");
  const [files, setFiles] = useState<Array<{ path: string; content: string }>>(
    [],
  );
  const [diffInfo, setDiffInfo] = useState<{
    summary: string;
    diffs: string;
    ops: Array<FileOperation>;
  }>({ summary: "", diffs: "", ops: [] });
  const [, setShowSpinner] = useState(false);
  const [applyOps, setApplyOps] = useState<Array<FileOperation>>([]);
  const [quietExit, setQuietExit] = useState(false);
  const [showDirInfo, setShowDirInfo] = useState(false);
  const contextLimit = MAX_CONTEXT_CHARACTER_LIMIT;
  const inputPromptValueRef = useRef<string>("");

  /* ---------------------------- Load file context --------------------------- */
  useEffect(() => {
    (async () => {
      const ignorePats = loadIgnorePatterns();
      const fileContents = await getFileContents(rootPath, ignorePats);
      setFiles(fileContents);
    })();
  }, [rootPath]);

  useEffect(() => {
    if (files.length) {
      setState("prompt");
    }
  }, [files]);

  /* -------------------------------- Helpers -------------------------------- */

  async function runSinglePassTask(userPrompt: string) {
    setPrompt(userPrompt);
    setShowSpinner(true);
    setState("thinking");

    try {
      const taskContextStr = renderTaskContext({
        prompt: userPrompt,
        input_paths: [rootPath],
        input_paths_structure: "(omitted for brevity in single pass mode)",
        files,
      });

      const openai = new OpenAI({
        apiKey: config.apiKey ?? "",
        baseURL: OPENAI_BASE_URL || undefined,
        timeout: OPENAI_TIMEOUT_MS,
      });
      const chatResp = await openai.beta.chat.completions.parse({
        model: config.model,
        ...(config.flexMode ? { service_tier: "flex" } : {}),
        messages: [
          {
            role: "user",
            content: taskContextStr,
          },
        ],
        response_format: zodResponseFormat(EditedFilesSchema, "schema"),
      });

      const edited = chatResp.choices[0]?.message?.parsed ?? null;

      setShowSpinner(false);

      if (!edited || !Array.isArray(edited.ops)) {
        setState("noops");
        return;
      }

      const originalMap: Record<string, string> = {};
      for (const fc of files) {
        originalMap[fc.path] = fc.content;
      }

      const [combinedDiffs, opsToApply] = generateDiffSummary(
        edited,
        originalMap,
      );

      if (!opsToApply.length) {
        setState("noops");
        return;
      }

      const summary = generateEditSummary(opsToApply, originalMap);
      setDiffInfo({ summary, diffs: combinedDiffs, ops: opsToApply });
      setApplyOps(opsToApply);
      setState("confirm");
    } catch (err) {
      setShowSpinner(false);
      setState("error");
    }
  }

  async function applyFileOps(ops: Array<FileOperation>) {
    for (const op of ops) {
      if (op.delete) {
        try {
          await fsPromises.unlink(op.path);
        } catch {
          /* ignore */
        }
      } else if (op.move_to) {
        const newContent = op.updated_full_content || "";
        try {
          await fsPromises.mkdir(path.dirname(op.move_to), { recursive: true });
          await fsPromises.writeFile(op.move_to, newContent, "utf-8");
        } catch {
          /* ignore */
        }
        try {
          await fsPromises.unlink(op.path);
        } catch {
          /* ignore */
        }
      } else {
        const newContent = op.updated_full_content || "";
        try {
          await fsPromises.mkdir(path.dirname(op.path), { recursive: true });
          await fsPromises.writeFile(op.path, newContent, "utf-8");
        } catch {
          /* ignore */
        }
      }
    }
    setState("applied");
  }

  /* --------------------------------- Render -------------------------------- */

  useInput((_input, key) => {
    if (state === "applied") {
      setState("prompt");
    } else if (
      (key.ctrl && (_input === "c" || _input === "C")) ||
      _input === "\u0003"
    ) {
      // If in thinking mode, treat this as an interrupt and reset to prompt
      if (state === "thinking") {
        setState("interrupted");
        // If you want to exit the process altogether instead:
        // app.exit();
        // if (onExit) onExit();
      } else if (state === "prompt") {
        // Ctrl+C in prompt mode quits
        app.exit();
        if (onExit) {
          onExit();
        }
      }
    }
  });

  if (quietExit) {
    setTimeout(() => {
      onExit && onExit();
      app.exit();
    }, 100);
    return <Text>Session complete.</Text>;
  }

  if (state === "init") {
    return (
      <Box flexDirection="column">
        <Text>Directory: {rootPath}</Text>
        <Text color="gray">Loading file context…</Text>
      </Box>
    );
  }

  if (state === "error") {
    return (
      <Box flexDirection="column">
        <Text color="red">Error calling OpenAI API.</Text>
        <ContinuePrompt
          onResult={(cont) => {
            if (!cont) {
              setQuietExit(true);
            } else {
              setState("prompt");
            }
          }}
        />
      </Box>
    );
  }

  if (state === "noops") {
    return (
      <Box flexDirection="column">
        <Text color="yellow">No valid operations returned.</Text>
        <ContinuePrompt
          onResult={(cont) => {
            if (!cont) {
              setQuietExit(true);
            } else {
              setState("prompt");
            }
          }}
        />
      </Box>
    );
  }

  if (state === "applied") {
    return (
      <Box flexDirection="column">
        <Text color="green">Changes have been applied.</Text>
        <Text color="gray">Press any key to continue…</Text>
      </Box>
    );
  }

  if (state === "thinking") {
    return <WorkingSpinner />;
  }

  if (state === "interrupted") {
    // Reset prompt input value (clears what was typed before interruption)
    inputPromptValueRef.current = "";
    setTimeout(() => setState("prompt"), 250);
    return (
      <Box flexDirection="column">
        <Text color="red">
          Interrupted. Press Enter to return to prompt mode.
        </Text>
      </Box>
    );
  }

  if (state === "prompt") {
    return (
      <Box flexDirection="column" gap={1}>
        {/* Info Box */}
        <Box borderStyle="round" flexDirection="column" paddingX={1} width={80}>
          <Text>
            <Text bold color="magenta">
              OpenAI <Text bold>Codex</Text>
            </Text>{" "}
            <Text dimColor>(full context mode)</Text>
          </Text>
          <Text>
            <Text bold color="greenBright">
              →
            </Text>{" "}
            <Text bold>Model:</Text> {config.model}
          </Text>
        </Box>

        {/* Directory info */}
        <DirectoryInfo
          rootPath={rootPath}
          files={files}
          contextLimit={contextLimit}
          showStruct={showDirInfo}
        />

        {/* Prompt Input Box */}
        <Box borderStyle="round" paddingX={1}>
          <InputPrompt
            message=">>> "
            onSubmit={(val) => {
              // Support /context as a command to show the directory structure.
              if (val === "/context" || val === ":context") {
                setShowDirInfo(true);
                setPrompt("");
                return;
              } else {
                setShowDirInfo(false);
              }

              // Continue if prompt is empty.
              if (!val) {
                return;
              }

              runSinglePassTask(val);
            }}
            onCtrlC={() => {
              setState("interrupted");
            }}
          />
        </Box>

        <Box marginTop={1}>
          <Text dimColor>
            {"Type /context to display the directory structure."}
          </Text>
          <Text dimColor>
            {" Press Ctrl+C at any time to interrupt / exit."}
          </Text>
        </Box>
      </Box>
    );
  }

  if (state === "confirm") {
    return (
      <Box flexDirection="column">
        <SummaryAndDiffs summary={diffInfo.summary} diffs={diffInfo.diffs} />
        <ConfirmationPrompt
          message="Apply these changes?"
          onResult={(accept) => {
            if (accept) {
              applyFileOps(applyOps);
            } else {
              setState("skipped");
            }
          }}
        />
      </Box>
    );
  }

  if (state === "skipped") {
    setTimeout(() => {
      setState("prompt");
    }, 0);

    return (
      <Box flexDirection="column">
        <Text color="red">Skipped proposed changes.</Text>
      </Box>
    );
  }

  return <Text color="gray">…</Text>;
}

export default {};
