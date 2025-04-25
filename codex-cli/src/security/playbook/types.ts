import { z } from "zod";

// Define the schema for a Playbook and its Steps
export const PlaybookSchema = z.object({
  id: z.string(),
  name: z.string().optional(),
  mode: z.literal("predator"),
  retry_on_failure: z.boolean().optional(),
  steps: z.array(z.object({
    id: z.string().optional(),
    phase: z.string(),
    description: z.string().optional(),
    action: z.object({
      method: z.enum(["GET","POST","PUT","PATCH","DELETE","HEAD","OPTIONS"]),
      path: z.string(),
    }).optional(),
    headers: z.record(z.string()).optional(),
    payload: z.any().optional(),
    extract: z.object({ path: z.string(), save_as: z.string() }).optional(),
    validate: z.object({ status_code: z.number().optional(), contains: z.string().optional() }).optional(),
    retry_on_failure: z.boolean().optional(),
    // Optional Puppeteer browser automation block
    puppeteer: z.object({
      // Optional URL (relative or absolute) to navigate first
      url: z.string().optional(),
      actions: z.array(
        z.discriminatedUnion("type", [
          z.object({ type: z.literal("type"), selector: z.string(), text: z.string() }),
          z.object({ type: z.literal("click"), selector: z.string() }),
          z.object({ type: z.literal("waitForNavigation"), options: z.any().optional() }),
          z.object({ type: z.literal("extractCookie"), name: z.string(), save_as: z.string() }),
        ])
      )
    }).optional(),
  })),
});

/**
 * TypeScript types inferred from the playbook schema.
 */
export type Playbook = z.infer<typeof PlaybookSchema>;
export type Step = Playbook["steps"][number];