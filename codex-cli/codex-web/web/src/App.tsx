import AddModelModal from "./components/AddModelModal";
import NewProviderModal from "./components/NewProviderModal";
import { useModels } from "./hooks/useModels";
import { useProviders } from "./hooks/useProviders";
import {
  Box,
  Button,
  Flex,
  Heading,
  Select,
  Spinner,
  Text,
  VStack,
  useDisclosure,
  useToast,
} from "@chakra-ui/react";
import { useState } from "react";

type Provider = {
  name: string;
};

type Model = {
  id: string;
  ctx: number;
};

export default function App(): JSX.Element {
  const { providers, isLoading: loadingProv } = useProviders();

  const [provider, setProvider] = useState("openai");
  const { models, isLoading: loadingModels } = useModels(provider);
  const [model, setModel] = useState("");

  const toast = useToast();

  const newProv = useDisclosure();
  const addModel = useDisclosure();

  const save = async () => {
    await fetch("http://localhost:8787/config", {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ provider, model }),
    });
    toast({ title: "Defaults saved", status: "success", duration: 2500 });
  };

  return (
    <Flex h="100vh" align="center" justify="center" bg="gray.50">
      <Box bg="white" p={8} rounded="lg" shadow="md" w="400px">
        <VStack spacing={4} align="stretch">
          <Heading size="md">Codex Model Picker</Heading>

          <Button
            size="sm"
            variant="outline"
            alignSelf="flex-end"
            onClick={newProv.onOpen}
          >
            + New provider
          </Button>

          {loadingProv ? (
            <Spinner />
          ) : (
            <Select
              value={provider}
              onChange={(e) => {
                setProvider(e.target.value);
                setModel("");
              }}
            >
              {Object.entries(providers as Record<string, Provider>).map(
                ([id, p]) => (
                  <option key={id} value={id}>
                    {p.name}
                  </option>
                ),
              )}
            </Select>
          )}

          {loadingModels ? (
            <Spinner />
          ) : (
            <>
              <Select
                placeholder="Select model"
                value={model}
                onChange={(e) => setModel(e.target.value)}
              >
                {models.map((m: Model) => (
                  <option key={m.id} value={m.id}>
                    {m.id} ({Math.round(m.ctx / 1000)}k)
                  </option>
                ))}
              </Select>
              {models.length === 0 && (
                <Button size="sm" variant="ghost" onClick={addModel.onOpen}>
                  + Add model manually
                </Button>
              )}
            </>
          )}

          <Button colorScheme="teal" onClick={save} isDisabled={!model}>
            Save as default
          </Button>
          <Text fontSize="sm" color="gray.500">
            Defaults persist to ~/.codex/config.json
          </Text>
        </VStack>
      </Box>

      {/* Modals */}
      <NewProviderModal isOpen={newProv.isOpen} onClose={newProv.onClose} />
      <AddModelModal
        isOpen={addModel.isOpen}
        onClose={addModel.onClose}
        providerId={provider}
      />
    </Flex>
  );
}
