import type { ReviewDecision } from "../utils/agent/review";
import type React from "react";

import { useState, useCallback, useRef } from "react";

type ConfirmationResult = {
  decision: ReviewDecision;
  customDenyMessage?: string;
};

type ConfirmationItem = {
  prompt: React.ReactNode;
  resolve: (result: ConfirmationResult) => void;
  explanation?: string;
};

export function useConfirmation(): {
  submitConfirmation: (result: ConfirmationResult) => void;
  requestConfirmation: (
    prompt: React.ReactNode,
    explanation?: string,
  ) => Promise<ConfirmationResult>;
  confirmationPrompt: React.ReactNode | null;
  explanation?: string;
} {
  // The current prompt is just the head of the queue
  const [current, setCurrent] = useState<ConfirmationItem | null>(null);
  // The entire queue is stored in a ref to avoid re-renders
  const queueRef = useRef<Array<ConfirmationItem>>([]);

  // Move queue forward to the next prompt
  const advanceQueue = useCallback(() => {
    const next = queueRef.current.shift() ?? null;
    setCurrent(next);
  }, []);

  // Called whenever someone wants a confirmation
  const requestConfirmation = useCallback(
    (prompt: React.ReactNode, explanation?: string) => {
      return new Promise<ConfirmationResult>((resolve) => {
        const wasEmpty = queueRef.current.length === 0;
        queueRef.current.push({ prompt, resolve, explanation });

        // If the queue was empty, we need to kick off the first prompt
        if (wasEmpty) {
          advanceQueue();
        }
      });
    },
    [advanceQueue],
  );

  // Called whenever user picks Yes / No
  const submitConfirmation = (result: ConfirmationResult) => {
    if (current) {
      current.resolve(result);
      advanceQueue();
    }
  };

  return {
    confirmationPrompt: current?.prompt, // the prompt to render now
    explanation: current?.explanation, // the explanation to render if available
    requestConfirmation,
    submitConfirmation,
  };
}
