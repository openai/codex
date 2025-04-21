import React from 'react';
import { test, expect } from 'vitest';
import { render } from 'ink-testing-library';
import { Text } from 'ink';
import { TerminalChatCommandReview } from '../src/components/chat/terminal-chat-command-review';
import { ReviewDecision } from '../src/utils/agent/review';

// Ensure that pressing 's' triggers the onSwitchApprovalMode callback
test('pressing s triggers onSwitchApprovalMode', (done) => {
  const onSwitchApprovalMode = () => {
    done();
  };
  // onReviewCommand should not be called in this scenario
  const onReviewCommand = () => {
    done(new Error('onReviewCommand should not be called'));
  };
  const { stdin } = render(
    <TerminalChatCommandReview
      confirmationPrompt={<Text>Prompt</Text>}
      onReviewCommand={onReviewCommand}
      onSwitchApprovalMode={onSwitchApprovalMode}
    />
  );
  stdin.write('s');
});

// Ensure that pressing 'y' triggers onReviewCommand with YES decision
test('pressing y triggers onReviewCommand with YES', (done) => {
  const onReviewCommand = (decision: ReviewDecision) => {
    try {
      expect(decision).toBe(ReviewDecision.YES);
      done();
    } catch (err) {
      done(err as Error);
    }
  };
  // onSwitchApprovalMode should not be called in this scenario
  const onSwitchApprovalMode = () => {
    done(new Error('onSwitchApprovalMode should not be called'));
  };
  const { stdin } = render(
    <TerminalChatCommandReview
      confirmationPrompt={<Text>Prompt</Text>}
      onReviewCommand={onReviewCommand}
      onSwitchApprovalMode={onSwitchApprovalMode}
    />
  );
  stdin.write('y');
});