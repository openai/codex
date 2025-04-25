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
}: {
  confirmationPrompt: React.ReactNode;
  onReviewCommand: (decision: ReviewDecision, customMessage?: string) => void;
}): React.ReactElement {
  const [mode, setMode] = React.useState<"select" | "input">("select");
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

  useInput((input, key) => {
    if (mode === "select") {
      if (input === "y") {
        onReviewCommand(ReviewDecision.YES);
      } else if (input === "e") {
        setMode("input");
      } else if (input === "n") {
        onReviewCommand(
          ReviewDecision.NO_CONTINUE,
          "Don't do that, keep going though",
        );
      } else if (input === "a" && showAlwaysApprove) {
        onReviewCommand(ReviewDecision.ALWAYS);
      } else if (key.escape) {
        onReviewCommand(ReviewDecision.NO_EXIT);
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
        {mode === "select" ? (
          <>
            <Text>Allow command?</Text>
            <Box paddingX={2} flexDirection="column" gap={1}>
              <Select
                onChange={(value: ReviewDecision | "edit") => {
                  if (value === "edit") {
                    setMode("input");
                  } else {
                    onReviewCommand(value);
                  }
                }}
                options={approvalOptions}
              />
            </Box>
          </>
        ) : (
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
        )}
      </Box>
    </Box>
  );
}
