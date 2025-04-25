import type { ResponseInputItem } from "openai/resources/responses/responses";

import { fileTypeFromBuffer } from "file-type";
import fs from "fs/promises";

export async function createInputItem(
  text: string,
  images: Array<string>,
): Promise<ResponseInputItem.Message> {
  const inputItem: ResponseInputItem.Message = {
    role: "user",
    content: [{ type: "input_text", text }],
    type: "message",
  };

  for (const filePath of images) {
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
  }

  return inputItem;
}
