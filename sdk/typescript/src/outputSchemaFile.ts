import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";

export type OutputSchemaFile = {
  schemaPath?: string;
  cleanup: () => Promise<void>;
};

export async function createOutputSchemaFile(schema: unknown): Promise<OutputSchemaFile> {
  if (schema === undefined) {
    return { schemaPath: undefined, cleanup: async () => {} };
  }

  if (!isJsonObject(schema)) {
    throw new Error("outputSchema must be a plain JSON object");
  }

  const schemaDir = await fs.mkdtemp(path.join(os.tmpdir(), "codex-output-schema-"));
  const schemaPath = path.join(schemaDir, "schema.json");
  const cleanup = async () => {
    await fs.rm(schemaDir, { recursive: true, force: true });
  };

  try {
    await fs.writeFile(schemaPath, JSON.stringify(schema), "utf8");
    return { schemaPath, cleanup };
  } catch (error) {
    await cleanup();
    throw error;
  }
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
