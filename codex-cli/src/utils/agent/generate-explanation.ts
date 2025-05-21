import type { AppConfig } from "../config";

import { formatCommandForDisplay } from "../../format-command";
import { log } from "../../utils/logger/log.js";
import { createOpenAIClient } from "../openai-client";

/**
 * Generates an explanation for a shell command using the OpenAI API.
 *
 * @param command The command to explain
 * @param model The model to use for generating the explanation
 * @param flexMode Whether to use the flex-mode service tier
 * @param config The configuration object
 * @returns A human-readable explanation of what the command does
 */
export async function generateCommandExplanation(
  command: Array<string>,
  model: string,
  flexMode: boolean,
  config: AppConfig,
): Promise<string> {
  try {
    // Create a temporary OpenAI client
    const oai = createOpenAIClient(config);

    // Format the command for display
    const commandForDisplay = formatCommandForDisplay(command);

    // Create a prompt that asks for an explanation with a more detailed system prompt
    const response = await oai.chat.completions.create({
      model,
      ...(flexMode ? { service_tier: "flex" } : {}),
      messages: [
        {
          role: "system",
          content:
            "You are an expert in shell commands and terminal operations. Your task is to provide detailed, accurate explanations of shell commands that users are considering executing. Break down each part of the command, explain what it does, identify any potential risks or side effects, and explain why someone might want to run it. Be specific about what files or systems will be affected. If the command could potentially be harmful, make sure to clearly highlight those risks.",
        },
        {
          role: "user",
          content: `Please explain this shell command in detail: \`${commandForDisplay}\`\n\nProvide a structured explanation that includes:\n1. A brief overview of what the command does\n2. A breakdown of each part of the command (flags, arguments, etc.)\n3. What files, directories, or systems will be affected\n4. Any potential risks or side effects\n5. Why someone might want to run this command\n\nBe specific and technical - this explanation will help the user decide whether to approve or reject the command.`,
        },
      ],
    });

    // Extract the explanation from the response
    const explanation =
      response.choices[0]?.message.content || "Unable to generate explanation.";
    return explanation;
  } catch (error) {
    log(`Error generating command explanation: ${error}`);

    let errorMessage = "Unable to generate explanation due to an error.";
    if (error instanceof Error) {
      errorMessage = `Unable to generate explanation: ${error.message}`;

      // If it's an API error, check for more specific information
      if ("status" in error && typeof error.status === "number") {
        // Handle API-specific errors
        if (error.status === 401) {
          errorMessage =
            "Unable to generate explanation: API key is invalid or expired.";
        } else if (error.status === 429) {
          errorMessage =
            "Unable to generate explanation: Rate limit exceeded. Please try again later.";
        } else if (error.status >= 500) {
          errorMessage =
            "Unable to generate explanation: OpenAI service is currently unavailable. Please try again later.";
        }
      }
    }

    return errorMessage;
  }
}
