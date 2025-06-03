import type { ResponseInputItem } from "openai/resources/responses/responses";

import {
  resolveSnippetsInPrompt,
  hasSnippetReferences,
} from "./snippet-resolver.js";
import { fileTypeFromBuffer } from "file-type";
import fs from "fs/promises";
import path from "path";

export async function createInputItem(
  text: string,
  images: Array<string>,
): Promise<ResponseInputItem.Message> {
  // Resolve snippet references before creating the input item
  let processedText = text;

  if (hasSnippetReferences(text)) {
    const resolutionResult = resolveSnippetsInPrompt(text);
    processedText = resolutionResult.resolvedPrompt;

    // Log warnings if any snippets were not found
    if (resolutionResult.warnings.length > 0) {
      // eslint-disable-next-line no-console
      console.warn("⚠️  Snippet warnings:");
      resolutionResult.warnings.forEach((warning) => {
        // eslint-disable-next-line no-console
        console.warn(`   ${warning}`);
      });
    }

    // Log successful replacements for user feedback
    const successfulReplacements = resolutionResult.replacedSnippets.filter(
      (r) => r.found,
    );
    if (successfulReplacements.length > 0) {
      // eslint-disable-next-line no-console
      console.log(
        "✅ Expanded snippets:",
        successfulReplacements.map((r) => r.label).join(", "),
      );
    }
  }

  const inputItem: ResponseInputItem.Message = {
    role: "user",
    content: [{ type: "input_text", text: processedText }],
    type: "message",
  };

  for (const filePath of images) {
    try {
      /* eslint-disable no-await-in-loop */
      const binary = await fs.readFile(filePath);
      const kind = await fileTypeFromBuffer(binary);
      /* eslint-enable no-await-in-loop */
      const encoded = binary.toString("base64");
      const mime = kind?.mime ?? "application/octet-stream";
      inputItem.content.push({
        type: "input_image",
        detail: "auto",
        image_url: `data:${mime};base64,${encoded}`,
      });
    } catch (err) {
      inputItem.content.push({
        type: "input_text",
        text: `[missing image: ${path.basename(filePath)}]`,
      });
    }
  }

  return inputItem;
}
