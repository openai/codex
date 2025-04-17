import { ReviewDecision } from "../../utils/agent/review";
// TODO: figure out why `cli-spinners` fails on Node v20.9.0
// which is why we have to do this in the first place
//
// @ts-expect-error select.js is JavaScript and has no types
import { Select } from "../vendor/ink-select/select";
import TextInput from "../vendor/ink-text-input";
import { Box, Text, useInput } from "ink";
import React from "react";

// default deny‑reason:
const DEFAULT_DENY_MESSAGE =
  "Don't do that, but keep trying to fix the problem";

export function TerminalChatCommandReview({
  confirmationPrompt,
  onReviewCommand,
  explanation: propExplanation,
}: {
  confirmationPrompt: React.ReactNode;
  onReviewCommand: (decision: ReviewDecision, customMessage?: string) => void;
  explanation?: string;
}): React.ReactElement {

  const [selection, setSelection] = React.useState<ReviewDecision | "edit" | null>(null);
  const [mode, setMode] = React.useState<"select" | "input" | "explanation" | "confirm" >(
    "select",
  );
  const [explanation, setExplanation] = React.useState<string>("");

  // If the component receives an explanation prop, update the state
  React.useEffect(() => {
    if (propExplanation) {
      setExplanation(propExplanation);
      setMode("explanation");
    }
  }, [propExplanation]);

  const [msg, setMsg] = React.useState<string>("");

  // -------------------------------------------------------------------------
  // Determine whether the "always approve" option should be displayed.  We
  // only hide it for the special `apply_patch` command since approving those
  // permanently would bypass the user's review of future file modifications.
  // The information is embedded in the `confirmationPrompt` React element –
  // we inspect the `commandForDisplay` prop exposed by
  // <TerminalChatToolCallCommand/> to extract the base command.
  // -------------------------------------------------------------------------

  const showAlwaysApprove = React.useMemo(() => {
    if (
      React.isValidElement(confirmationPrompt) &&
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      typeof (confirmationPrompt as any).props?.commandForDisplay === "string"
    ) {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const command: string = (confirmationPrompt as any).props
        .commandForDisplay;
      // Grab the first token of the first line – that corresponds to the base
      // command even when the string contains embedded newlines (e.g. diffs).
      const baseCmd = command.split("\n")[0]?.trim().split(/\s+/)[0] ?? "";
      return baseCmd !== "apply_patch";
    }
    // Default to showing the option when we cannot reliably detect the base
    // command.
    return true;
  }, [confirmationPrompt]);

  // Memoize the list of selectable options to avoid recreating the array on
  // every render.  This keeps <Select/> stable and prevents unnecessary work
  // inside Ink.
  const approvalOptions = React.useMemo(() => {
    const opts: Array<
      | { label: string; value: ReviewDecision }
      | { label: string; value: "edit" }
    > = [
      {
        label: "Yes (y)",
        value: ReviewDecision.YES,
      },
    ];

    if (showAlwaysApprove) {
      opts.push({
        label: "Yes, always approve this exact command for this session (a)",
        value: ReviewDecision.ALWAYS,
      });
    }

    opts.push(
      {
        label: "Explain this command (x)",
        value: ReviewDecision.EXPLAIN,
      },
      {
        label: "Edit or give feedback (e)",
        value: "edit",
      },
      {
        label: "No, and keep going (n)",
        value: ReviewDecision.NO_CONTINUE,
      },
      {
        label: "No, and stop for now (esc)",
        value: ReviewDecision.NO_EXIT,
      },
    );

    return opts;
  }, [showAlwaysApprove]);


  // Added a confirm option so if user enters a wrong command they can confirm the comand or type /back to go back to the prev menu NOTE(@3xpl0itk1t)
  useInput((input, key) => {
    if (mode === "select") {
      if (input === "y") {
        setSelection(ReviewDecision.YES);
        setMode("confirm");
        onReviewCommand(ReviewDecision.YES);
      } else if (input === "x") {
        onReviewCommand(ReviewDecision.EXPLAIN);
      } else if (input === "e") {
        setMode("input");
      } else if (input === "n") {
        setSelection(ReviewDecision.NO_CONTINUE);
        setMode("confirm");
        onReviewCommand(
          ReviewDecision.NO_CONTINUE,
          "Don't do that, keep going though",
        );
      } else if (input === "a" && showAlwaysApprove) {
        setSelection(ReviewDecision.ALWAYS);
        setMode("confirm");
        onReviewCommand(ReviewDecision.ALWAYS);
      } else if (key.escape) {
        setSelection(ReviewDecision.NO_EXIT);
        setMode("confirm");
        onReviewCommand(ReviewDecision.NO_EXIT);
      }
    } else if (mode === "confirm") {
      if (input === "/back") {
        setMode("select");
        setSelection(null);
      } else if (input === "y" || input === "yes") {
        if (selection !== null && selection !== "edit") {
          onReviewCommand(selection);
        }
      } else if (input === "n" || input === "no") {
        if (selection === ReviewDecision.NO_CONTINUE) {
          const customMessage = msg.trim() === "" ? DEFAULT_DENY_MESSAGE : msg;
          onReviewCommand(ReviewDecision.NO_CONTINUE, customMessage);
        } else {
          onReviewCommand(ReviewDecision.NO_EXIT);
        }
      }
    } else if (mode === "explanation") {
      // When in explanation mode, any key returns to select mode
      if (key.return || key.escape || input === "x") {
        setMode("select");
      }
    } else {
      // text entry mode
      if (key.return) {
        // if user hit enter on empty msg, fall back to DEFAULT_DENY_MESSAGE
        const custom = msg.trim() === "" ? DEFAULT_DENY_MESSAGE : msg;
        onReviewCommand(ReviewDecision.NO_CONTINUE, custom);
      } else if (key.escape) {
        // treat escape as denial with default message as well
        onReviewCommand(
          ReviewDecision.NO_CONTINUE,
          msg.trim() === "" ? DEFAULT_DENY_MESSAGE : msg,
        );
      }
    }
  });

  return (
    <Box flexDirection="column" gap={1} borderStyle="round" marginTop={1}>
      {confirmationPrompt}
      <Box flexDirection="column" gap={1}>
        {mode === "explanation" ? (
          <>
            <Text bold color="yellow">
              Command Explanation:
            </Text>
            <Box paddingX={2} flexDirection="column" gap={1}>
              {explanation ? (
                <>
                  {explanation.split("\n").map((line, i) => {
                    // Check if it's an error message
                    if (
                      explanation.startsWith("Unable to generate explanation")
                    ) {
                      return (
                        <Text key={i} bold color="red">
                          {line}
                        </Text>
                      );
                    }
                    // Apply different styling to headings (numbered items)
                    else if (line.match(/^\d+\.\s+/)) {
                      return (
                        <Text key={i} bold color="cyan">
                          {line}
                        </Text>
                      );
                    } else {
                      return <Text key={i}>{line}</Text>;
                    }
                  })}
                </>
              ) : (
                <Text dimColor>Loading explanation...</Text>
              )}
              <Text dimColor>Press any key to return to options</Text>
            </Box>
          </>
        ) : mode === "select" ? (
          <>
            <Text>Allow command?</Text>
            <Box paddingX={2} flexDirection="column" gap={1}>
              <Select
                onChange={(value: ReviewDecision | "edit") => {
                  if (value === "edit") {
                    setMode("input");
                  } else {
                    onReviewCommand(value as ReviewDecision);
                  }
                }}
                options={approvalOptions}
              />
            </Box>
          </>
        ) : mode === "confirm" ? (
          <>
            <Text>Confirm your choice (y to confirm, n to deny, /back to go back):</Text>
            <Text>
              You selected:{" "}
              {selection === ReviewDecision.YES
                ? "Yes"
                : selection === ReviewDecision.ALWAYS
                ? "Always approve"
                : selection === ReviewDecision.NO_CONTINUE
                ? "No, continue"
                : selection === ReviewDecision.NO_EXIT
                ? "No, exit"
                : "Unknown option"}
            </Text>
            <Text dimColor>Type "/back" to return to the selection menu.</Text>
          </>
        ) : mode === "input" ? (
          <>
            <Text>Give the model feedback (↵ to submit):</Text>
            <Box borderStyle="round">
              <Box paddingX={1}>
                <TextInput
                  value={msg}
                  onChange={setMsg}
                  placeholder="type a reason"
                  showCursor
                  focus
                />
              </Box>
            </Box>

            {msg.trim() === "" && (
              <Box paddingX={2} marginBottom={1}>
                <Text dimColor>
                  default:&nbsp;
                  <Text>{DEFAULT_DENY_MESSAGE}</Text>
                </Text>
              </Box>
            )}
          </>
        ) : null}
      </Box>
    </Box>
  );
}
