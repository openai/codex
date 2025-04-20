import type {
  ResponseItem,
  ResponseOutputItem,
} from "openai/resources/responses/responses.mjs";

/**
 * Build a GitHub issues‐new URL that pre‑fills the Codex 2‑bug‑report.yml
 * template with whatever structured data we can infer from the current
 * session.
 */
export function buildBugReportUrl({
  items,
  cliVersion,
  model,
  platform,
}: {
  /** Chat history so we can summarise user steps */
  items: Array<ResponseItem | ResponseOutputItem>;
  /** CLI revision string (e.g. output of `codex --revision`) */
  cliVersion: string;
  /** Active model name */
  model: string;
  /** Platform string – e.g. `darwin arm64 23.0.0` */
  platform: string;
}): string {
  const params = new URLSearchParams({
    template: "2-bug-report.yml",
    labels: "bug",
  });

  params.set("version", cliVersion);
  params.set("model", model);
  params.set("platform", platform);

  const bullets: Array<string> = [];
  for (let i = 0; i < items.length; ) {
    const entry = items[i];
    if (entry?.type === "message" && entry.role === "user") {
      const contentArray = entry.content as
        | Array<{ text?: string }>
        | undefined;
      const messageText = contentArray
        ?.map((c) => c.text ?? "")
        .join(" ")
        .trim();

      let reasoning = 0;
      let toolCalls = 0;
      let j = i + 1;
      while (j < items.length) {
        const it = items[j];
        if (it?.type === "message" && it?.role === "user") {
          break;
        } else if (
          it?.type === "reasoning" ||
          (it?.type === "message" && it?.role === "assistant")
        ) {
          reasoning += 1;
        } else if (it?.type === "function_call") {
          toolCalls += 1;
        }
        j++;
      }

      const codeBlock = `\`\`\`\n  ${messageText}\n  \`\`\``;

      bullets.push(
        `- ${codeBlock}\n  - \`${reasoning} reasoning\` | \`${toolCalls} tool\``,
      );

      i = j;
    } else {
      i += 1;
    }
  }

  if (bullets.length) {
    params.set("steps", bullets.join("\n"));
  }

  return `https://github.com/openai/codex/issues/new?${params.toString()}`;
}
