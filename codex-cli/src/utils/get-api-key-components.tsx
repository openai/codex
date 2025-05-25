import SelectInput from "../components/select-input/select-input.js";
import Spinner from "../components/vendor/ink-spinner.js";
import TextInput from "../components/vendor/ink-text-input.js";
import { Box, Text } from "ink";
import React, { useState } from "react";

export type Choice = { type: "signin" } | { type: "apikey"; key: string };

export function ApiKeyPrompt({
  onDone,
}: {
  onDone: (choice: Choice) => void;
}): JSX.Element {
  const [step, setStep] = useState<"select" | "paste">("select");
  const [apiKey, setApiKey] = useState("");

  if (step === "select") {
    return (
      <Box flexDirection="column" gap={1}>
        <Box flexDirection="column">
          <Text>
            Sign in with ChatGPT to generate an API key or paste one you already
            have.
          </Text>
          <Text dimColor>[use arrows to move, enter to select]</Text>
        </Box>
        <SelectInput
          items={[
            { label: "Sign in with ChatGPT", value: "signin" },
            {
              label: "Paste an API key (or set as OPENAI_API_KEY)",
              value: "paste",
            },
          ]}
          onSelect={(item: { value: string }) => {
            if (item.value === "signin") {
              onDone({ type: "signin" });
            } else {
              setStep("paste");
            }
          }}
        />
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text>Paste your OpenAI API key and press &lt;Enter&gt;:</Text>
      <TextInput
        value={apiKey}
        onChange={setApiKey}
        onSubmit={(value: string) => {
          if (value.trim() !== "") {
            onDone({ type: "apikey", key: value.trim() });
          }
        }}
        placeholder="sk-..."
        mask="*"
      />
    </Box>
  );
}

export function WaitingForAuth(): JSX.Element {
  return (
    <Box flexDirection="row" marginTop={1}>
      <Spinner type="ball" />
      <Text>
        {" "}
        Waiting for authentication… <Text dimColor>ctrl + c to quit</Text>
      </Text>
    </Box>
  );
}
