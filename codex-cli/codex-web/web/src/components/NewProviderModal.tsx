import {
  Modal,
  ModalBody,
  ModalCloseButton,
  ModalContent,
  ModalFooter,
  ModalHeader,
  ModalOverlay,
  Button,
  FormControl,
  FormLabel,
  Input,
  Stack,
  useToast,
} from "@chakra-ui/react";
import React, { useState } from "react";
import { mutate } from "swr";

type Props = {
  isOpen: boolean;
  onClose: () => void;
};

export default function NewProviderModal({
  isOpen,
  onClose,
}: Props): JSX.Element {
  const toast = useToast();

  const [form, setForm] = useState({
    id: "",
    name: "",
    baseURL: "",
    envKey: "",
    apiKey: "",
  });

  const onChange = (k: string) => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [k]: e.target.value });

  const save = async () => {
    const { id, name, baseURL, envKey, apiKey } = form;

    await fetch("http://localhost:8787/providers", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id, name, baseURL, envKey }),
    });

    if (apiKey) {
      await fetch(`http://localhost:8787/providers/${id}/key`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ key: apiKey }),
      });
    }

    mutate("http://localhost:8787/providers");
    toast({ title: "Provider added", status: "success", duration: 2500 });
    onClose();
  };

  const { id, name, baseURL, envKey } = form;

  return (
    <Modal isOpen={isOpen} onClose={onClose} size="lg">
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>Add new provider</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <Stack spacing={3}>
            <FormControl isRequired>
              <FormLabel>ID (slug)</FormLabel>
              <Input value={id} onChange={onChange("id")} />
            </FormControl>
            <FormControl isRequired>
              <FormLabel>Name</FormLabel>
              <Input value={name} onChange={onChange("name")} />
            </FormControl>
            <FormControl isRequired>
              <FormLabel>Base URL</FormLabel>
              <Input value={baseURL} onChange={onChange("baseURL")} />
            </FormControl>
            <FormControl isRequired>
              <FormLabel>ENV key (e.g. OPENAI_API_KEY)</FormLabel>
              <Input value={envKey} onChange={onChange("envKey")} />
            </FormControl>
            <FormControl>
              <FormLabel>API key (saved server-side)</FormLabel>
              <Input value={form.apiKey} onChange={onChange("apiKey")} />
            </FormControl>
          </Stack>
        </ModalBody>
        <ModalFooter>
          <Button mr={3} onClick={onClose}>
            Cancel
          </Button>
          <Button
            colorScheme="teal"
            onClick={save}
            isDisabled={!id || !name || !baseURL || !envKey}
          >
            Save
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
