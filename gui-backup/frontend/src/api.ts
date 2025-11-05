import axios from "axios";
import type {
  ActionMetadata,
  ExecuteActionPayload,
  ExecutionResponse
} from "./types";

const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8787";

const client = axios.create({
  baseURL: API_URL,
  headers: {
    "Content-Type": "application/json"
  },
  timeout: 60_000
});

export async function fetchActions(): Promise<ActionMetadata[]> {
  const { data } = await client.get<ActionMetadata[]>("/api/actions");
  return data;
}

export async function executeAction(
  actionId: string,
  payload: ExecuteActionPayload
): Promise<ExecutionResponse> {
  const { data } = await client.post<ExecutionResponse>(
    `/api/actions/${actionId}/execute`,
    payload
  );
  return data;
}
