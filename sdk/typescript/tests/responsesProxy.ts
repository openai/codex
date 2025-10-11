import http from "node:http";

const DEFAULT_RESPONSE_ID = "resp_mock";
const DEFAULT_MESSAGE_ID = "msg_mock";

type ResponseCompletedUsage = {
  input_tokens: number;
  input_tokens_details: { cached_tokens: number } | null;
  output_tokens: number;
  output_tokens_details: { reasoning_tokens: number } | null;
  total_tokens: number;
};

const DEFAULT_COMPLETED_USAGE: ResponseCompletedUsage = {
  input_tokens: 42,
  input_tokens_details: { cached_tokens: 12 },
  output_tokens: 5,
  output_tokens_details: null,
  total_tokens: 47,
};

type SseEvent = {
  type: string;
  [key: string]: unknown;
};

type SseResponseBody = {
  kind: "sse";
  events: SseEvent[];
};

export type ResponsesProxyOptions = {
  responseBodies: SseResponseBody[];
  statusCode?: number;
};

export type ResponsesProxy = {
  url: string;
  close: () => Promise<void>;
  requests: RecordedRequest[];
};

export type ResponsesApiRequest = {
  model?: string;
  input: Array<{
    role: string;
    content?: Array<{ type: string; text: string }>;
  }>;
  text?: {
    format?: Record<string, unknown>;
  };
};

export type RecordedRequest = {
  body: string;
  json: ResponsesApiRequest;
  headers: http.IncomingHttpHeaders;
};

function formatSseEvent(event: SseEvent): string {
  return `event: ${event.type}\n` + `data: ${JSON.stringify(event)}\n\n`;
}

export async function startResponsesTestProxy(
  options: ResponsesProxyOptions,
): Promise<ResponsesProxy> {
  const responseBodies = options.responseBodies;
  if (responseBodies.length === 0) {
    throw new Error("responseBodies is required");
  }

  const requests: RecordedRequest[] = [];

  function readRequestBody(req: http.IncomingMessage): Promise<string> {
    return new Promise<string>((resolve, reject) => {
      const chunks: Buffer[] = [];
      req.on("data", (chunk) => {
        chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
      });
      req.on("end", () => {
        resolve(Buffer.concat(chunks).toString("utf8"));
      });
      req.on("error", reject);
    });
  }

  let responseIndex = 0;

  const server = http.createServer((req, res) => {
    async function handle(): Promise<void> {
      if (req.method === "POST" && req.url === "/responses") {
        const body = await readRequestBody(req);
        const json = JSON.parse(body);
        requests.push({ body, json, headers: { ...req.headers } });

        const status = options.statusCode ?? 200;
        res.statusCode = status;
        res.setHeader("content-type", "text/event-stream");

        const responseBody = responseBodies[Math.min(responseIndex, responseBodies.length - 1)]!;
        responseIndex += 1;
        for (const event of responseBody.events) {
          res.write(formatSseEvent(event));
        }
        res.end();
        return;
      }

      res.statusCode = 404;
      res.end();
    }

    handle().catch(() => {
      res.statusCode = 500;
      res.end();
    });
  });

  const url = await new Promise<string>((resolve, reject) => {
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("Unable to determine proxy address"));
        return;
      }
      server.off("error", reject);
      const info = address;
      resolve(`http://${info.address}:${info.port}`);
    });
    server.once("error", reject);
  });

  async function close(): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      server.close((err) => {
        if (err) {
          reject(err);
          return;
        }
        resolve();
      });
    });
  }
  return { url, close, requests };
}

export function sse(...events: SseEvent[]): SseResponseBody {
  return {
    kind: "sse",
    events,
  };
}

export function responseStarted(responseId: string = DEFAULT_RESPONSE_ID): SseEvent {
  return {
    type: "response.created",
    response: {
      id: responseId,
    },
  };
}

export function assistantMessage(text: string, itemId: string = DEFAULT_MESSAGE_ID): SseEvent {
  return {
    type: "response.output_item.done",
    item: {
      type: "message",
      role: "assistant",
      id: itemId,
      content: [
        {
          type: "output_text",
          text,
        },
      ],
    },
  };
}

export function responseFailed(errorMessage: string): SseEvent {
  return {
    type: "error",
    error: { code: "rate_limit_exceeded", message: errorMessage },
  };
}

export function responseCompleted(
  responseId: string = DEFAULT_RESPONSE_ID,
  usage: ResponseCompletedUsage = DEFAULT_COMPLETED_USAGE,
): SseEvent {
  const inputDetails = usage.input_tokens_details ? { ...usage.input_tokens_details } : null;
  const outputDetails = usage.output_tokens_details ? { ...usage.output_tokens_details } : null;
  return {
    type: "response.completed",
    response: {
      id: responseId,
      usage: {
        input_tokens: usage.input_tokens,
        input_tokens_details: inputDetails,
        output_tokens: usage.output_tokens,
        output_tokens_details: outputDetails,
        total_tokens: usage.total_tokens,
      },
    },
  };
}
