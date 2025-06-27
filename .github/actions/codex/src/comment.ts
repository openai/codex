import type { EnvContext } from "./env-context";
import { runCodex } from "./run-codex";
import { postComment } from "./post-comment";
import { addEyesReaction } from "./add-reaction";

/**
 * Handle `issue_comment` and `pull_request_review_comment` events once we know
 * the action is supported.
 */
export async function onComment(ctx: EnvContext): Promise<void> {
  const triggerPhrase = ctx.tryGet("INPUT_TRIGGER_PHRASE");
  if (!triggerPhrase) {
    console.warn("Empty trigger phrase: skipping.");
    return;
  }

  // Attempt to get the body of the comment from the environment. Depending on
  // the event type either `GITHUB_EVENT_COMMENT_BODY` (issue & PR comments) or
  // `GITHUB_EVENT_REVIEW_BODY` (PR reviews) is set.
  const commentBody =
    ctx.tryGetNonEmpty("GITHUB_EVENT_COMMENT_BODY") ??
    ctx.tryGetNonEmpty("GITHUB_EVENT_REVIEW_BODY") ??
    ctx.tryGetNonEmpty("GITHUB_EVENT_ISSUE_BODY");

  if (!commentBody) {
    console.warn("Comment body not found in environment: skipping.");
    return;
  }

  // Check if the trigger phrase is present.
  if (!commentBody.includes(triggerPhrase)) {
    console.log(
      `Trigger phrase '${triggerPhrase}' not found: nothing to do for this comment.`,
    );
    return;
  }

  // Derive the prompt by removing the trigger phrase. Remove only the first
  // occurrence to keep any additional occurrences that might be meaningful.
  const prompt = commentBody.replace(triggerPhrase, "").trim();

  if (prompt.length === 0) {
    console.warn("Prompt is empty after removing trigger phrase: skipping");
    return;
  }

  // Provide immediate feedback that we are working on the request.
  await addEyesReaction(ctx);

  // Run Codex and post the response as a new comment.
  const lastMessage = await runCodex(prompt, ctx);
  await postComment(lastMessage, ctx);
}
