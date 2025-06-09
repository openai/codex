import type { EnvContext } from "./env-context";
import { runCodex } from "./run-codex";
import { postComment } from "./post-comment";
import { addEyesReaction } from "./add-reaction";

/**
 * Handle `pull_request_review` events. We treat the review body the same way
 * as a normal comment.
 */
export async function onReview(ctx: EnvContext): Promise<void> {
  const triggerPhrase = ctx.tryGet("INPUT_TRIGGER_PHRASE");
  if (!triggerPhrase) {
    console.warn("Empty trigger phrase: skipping.");
    return;
  }

  const reviewBody = ctx.tryGet("GITHUB_EVENT_REVIEW_BODY");

  if (!reviewBody) {
    console.warn("Review body not found in environment: skipping.");
    return;
  }

  if (!reviewBody.includes(triggerPhrase)) {
    console.log(
      `Trigger phrase '${triggerPhrase}' not found: nothing to do for this review.`,
    );
    return;
  }

  const prompt = reviewBody.replace(triggerPhrase, "").trim();

  if (prompt.length === 0) {
    console.warn("Prompt is empty after removing trigger phrase: skipping.");
    return;
  }

  await addEyesReaction(ctx);

  const lastMessage = await runCodex(prompt, ctx);
  await postComment(lastMessage, ctx);
}
