// @ts-expect-error select.js is JavaScript and has no types
import { Select } from "../vendor/ink-select/select";
import { Box, Text } from "ink";
import React from "react";
import { AutoApprovalMode } from "src/utils/auto-approval-mode";

// TODO: figure out why `cli-spinners` fails on Node v20.9.0
// which is why we have to do this in the first place

export function OnboardingApprovalMode(): React.ReactElement {
  return (
    <Box>
      <Text>Choose what you want to have to approve:</Text>
      <Select
        onChange={() => {}}
        // onChange={(value: ReviewDecision) => onReviewCommand(value)}
        options={[
          {
            label: "Auto-approve file reads, but ask me for edits and commands",
            value: AutoApprovalMode.SUGGEST,
          },
          {
            label: "Auto-approve file reads and edits, but ask me for commands",
            value: AutoApprovalMode.AUTO_EDIT,
          },
          {
            label:
              "Auto-approve file reads, edits, and running commands network-disabled",
            value: AutoApprovalMode.FULL_AUTO,
          },
        ]}
      />
    </Box>
  );
}
