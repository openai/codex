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
import { useState } from "react";
import { mutate } from "swr";

type Props = {
  isOpen: boolean;
  onClose: () => void;
  providerId: string;
};

export default function AddModelModal({
  isOpen,
  onClose,
  providerId,
}: Props): JSX.Element {
  const toast = useToast();
  const [modelId, setModelId] = useState("");
  const [ctx, setCtx] = useState("");

  const save = async () => {
    await fetch(`http://localhost:8787/providers/${providerId}/models`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ modelId, ctx }),
    });

    mutate(`http://localhost:8787/providers/${providerId}/models`);
    toast({ title: "Model added", status: "success", duration: 2500 });
    onClose();
  };

  return (
    <Modal isOpen={isOpen} onClose={onClose}>
      <ModalOverlay />
      <ModalContent>
        <ModalHeader>Add model to {providerId}</ModalHeader>
        <ModalCloseButton />
        <ModalBody>
          <Stack spacing={3}>
            <FormControl isRequired>
              <FormLabel>Model ID</FormLabel>
              <Input
                value={modelId}
                onChange={(e) => setModelId(e.target.value)}
              />
            </FormControl>
            <FormControl>
              <FormLabel>Context length (tokens)</FormLabel>
              <Input value={ctx} onChange={(e) => setCtx(e.target.value)} />
            </FormControl>
          </Stack>
        </ModalBody>
        <ModalFooter>
          <Button mr={3} onClick={onClose}>
            Cancel
          </Button>
          <Button colorScheme="teal" onClick={save} isDisabled={!modelId}>
            Save
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
