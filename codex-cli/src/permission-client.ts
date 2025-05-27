import type { ConfirmationResult } from "./hooks/use-confirmation";
import type { Socket } from "socket.io-client";

import { ReviewDecision } from "./utils/agent/review";
import { io } from "socket.io-client";

/** ---- internal singleton socket & helpers -------------------------------- */

type PermissionRequestPayload = {
  agentId: string;
  message: string;
};

type PermissionResponsePayload = ConfirmationResult & {
  agentId: string;
};

let socket: Socket | null = null;
let activeRequest: {
  agentId: string;
  resolve: (value: ConfirmationResult) => void;
  reject: (reason?: Error) => void;
} | null = null;

function getSocket(serverUrl: string): Socket {
  if (socket) {
    return socket;
  }

  socket = io(serverUrl, {
    path: "/socket.io",
    autoConnect: true,
    reconnection: true,
    reconnectionAttempts: Infinity,
    reconnectionDelay: 1000,
    reconnectionDelayMax: 30000,
  });

  socket.on("permission_response", (data: PermissionResponsePayload): void => {
    if (!activeRequest || activeRequest.agentId !== data.agentId) {
      return;
    }

    if (!Object.values(ReviewDecision).includes(data.decision)) {
      activeRequest.reject(
        new Error(
          `Unexpected decision value from permission server: ${data.decision}. Expected one of: ${Object.values(ReviewDecision).join(", ")}`,
        ),
      );
    } else {
      activeRequest.resolve({
        decision: data.decision,
        customDenyMessage: data.customDenyMessage,
      });
    }

    activeRequest = null; // Clear active request
  });

  return socket;
}

function waitUntilConnected(sock: Socket): Promise<void> {
  return sock.connected
    ? Promise.resolve()
    : new Promise((res) => sock.once("connect", () => res()));
}

/** ---- public API --------------------------------------------------------- */

export async function requestRemotePermission(
  agentId: string,
  serverUrl: string,
  prompt: string,
  explanation?: string,
): Promise<ConfirmationResult> {
  const fullMessage = explanation ? `${prompt}\n\n${explanation}` : prompt;

  if (activeRequest) {
    return Promise.reject(
      new Error(
        "A permission request is already active. Wait for it to complete before making another.",
      ),
    );
  }

  const sock = getSocket(serverUrl);
  await waitUntilConnected(sock);

  return new Promise<ConfirmationResult>((resolve, reject) => {
    activeRequest = { agentId, resolve, reject };

    try {
      sock.emit("permission_request", {
        agentId,
        message: fullMessage,
      } as PermissionRequestPayload);
    } catch (err) {
      activeRequest = null;
      reject(
        new Error(
          `Failed to send permission request: ${(err as Error).message}`,
        ),
      );
    }
  });
}
