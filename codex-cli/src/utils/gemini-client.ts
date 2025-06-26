import {
  GoogleGenerativeAI,
  type GenerateContentRequest,
  type Model,
  type ListModelsParameters,
  type Pager,
} from "@google/genai";

let genAI: GoogleGenerativeAI | undefined;

/**
 * Initializes the Gemini client with the provided API key.
 * @param apiKey The API key for the Gemini API.
 */
export function initializeGeminiClient(apiKey: string): void {
  if (!apiKey) {
    throw new Error("Gemini API key is required for initialization.");
  }
  genAI = new GoogleGenerativeAI({ apiKey });
}

/**
 * Gets the initialized Gemini client instance.
 * Throws an error if the client has not been initialized.
 * @returns The initialized GoogleGenerativeAI instance.
 */
function getClient(): GoogleGenerativeAI {
  if (!genAI) {
    throw new Error(
      "Gemini client not initialized. Call initializeGeminiClient first.",
    );
  }
  return genAI;
}

/**
 * Lists available Gemini models.
 * @param params Optional parameters for listing models (e.g., pageSize).
 * @returns A promise that resolves to an array of Model objects.
 */
export async function listGeminiModels(
  params?: ListModelsParameters,
): Promise<Model[]> {
  const client = getClient();
  try {
    const modelPager: Pager<Model> = await client.models.list(params);
    const models: Model[] = [];
    // The Pager object itself is an async iterable
    for await (const modelPage of modelPager) {
      // modelPage here is an array of Model objects from one page
      models.push(...modelPage);
    }
    return models;
  } catch (error) {
    console.error("Error listing Gemini models:", error);
    throw error; // Re-throw the error for the caller to handle
  }
}

/**
 * Generates content using a specified Gemini model and prompt.
 * @param modelId The ID of the model to use (e.g., "gemini-2.5-pro-preview-06-05").
 * @param prompt The text prompt to send to the model.
 * @returns A promise that resolves to the generated content text.
 */
export async function generateGeminiContent(
  modelId: string,
  prompt: string,
): Promise<string | undefined> {
  const client = getClient();
  try {
    const model = client.getModel({ model: modelId });
    const request: GenerateContentRequest = {
      contents: [{ role: "user", parts: [{ text: prompt }] }],
    };
    const result = await model.generateContent(request);
    const response = result.response;
    return response.text();
  } catch (error) {
    console.error("Error generating Gemini content:", error);
    throw error; // Re-throw the error for the caller to handle
  }
}

/**
 * A simple example of how to use the client.
 * This part can be removed or modified for actual use.
 */
async function exampleUsage(): Promise<void> {
  const apiKey = process.env["GEMINI_API_KEY"]; // Ensure you have this in your environment
  if (!apiKey) {
    console.log(
      "Please set the GEMINI_API_KEY environment variable for the example.",
    );
    return;
  }

  try {
    initializeGeminiClient(apiKey);
    console.log("Gemini client initialized.");

    console.log("\nListing models...");
    // Optionally, you can control pagination, e.g., { pageSize: 10 }
    const models = await listGeminiModels();
    console.log(
      "Available models:",
      models.map((m) => m.name).join(", "),
    );

    // Find a model that supports generateContent, e.g., one of the gemini-pro models
    // The exact name might vary, check the list output.
    // For this example, we'll try to find a model with "generateContent" in its supportedGenerationMethods
    // and "gemini" in its name.
    const suitableModel = models.find(
      (m) =>
        m.name.includes("gemini") && // Or a more specific name like "gemini-1.5-flash"
        m.supportedGenerationMethods.includes("generateContent"),
    );

    if (suitableModel) {
      console.log(`\nGenerating content with model: ${suitableModel.name}...`);
      const prompt = "Explain what a language model is in one sentence.";
      const content = await generateGeminiContent(suitableModel.name, prompt);
      console.log("Generated content:", content);
    } else {
      console.log(
        "\nCould not find a suitable model for content generation in the list.",
      );
      console.log(
        "Please check the model list and ensure you have access to a model that supports 'generateContent'.",
      );
    }
  } catch (error) {
    console.error("Example usage failed:", error);
  }
}

// To run the example (optional, can be commented out or removed):
// exampleUsage();
