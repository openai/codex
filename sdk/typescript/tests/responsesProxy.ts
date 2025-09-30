import http from "node:http";

const DEFAULT_RESPONSE_ID = "resp_mock";
const DEFAULT_MESSAGE_ID = "msg_mock";

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
  input: Array<{
    role: string;
    content?: Array<{ type: string; text: string }>;
  }>;
};

export type RecordedRequest = {
  body: string;
  json: ResponsesApiRequest;
};

const formatSseEvent = (event: SseEvent): string => {
  return `event: ${event.type}\n` + `data: ${JSON.stringify(event)}\n\n`;
};

export async function startResponsesTestProxy(
  options: ResponsesProxyOptions,
): Promise<ResponsesProxy> {
  const responseBodies = options.responseBodies;
  if (responseBodies.length === 0) {
    throw new Error("responseBodies is required");
  }

  const requests: RecordedRequest[] = [];

  const readRequestBody = (req: http.IncomingMessage) =>
    new Promise<string>((resolve, reject) => {
      const chunks: Buffer[] = [];
      req.on("data", (chunk) => {
        chunks.push(typeof chunk === "string" ? Buffer.from(chunk) : chunk);
      });
      req.on("end", () => {
        resolve(Buffer.concat(chunks).toString("utf8"));
      });
      req.on("error", reject);
    });

  let responseIndex = 0;

  const server = http.createServer((req, res) => {
    const handle = async () => {
      if (req.method === "POST" && req.url === "/responses") {
        const body = await readRequestBody(req);
        const json = JSON.parse(body);
        requests.push({ body, json });

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
    };

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

  const close = async () => {
    await new Promise<void>((resolve, reject) => {
      server.close((err) => {
        if (err) {
          reject(err);
          return;
        }
        resolve();
      });
    });
  };
  return { url, close, requests };
}

export const sse = (...events: SseEvent[]): SseResponseBody => ({
  kind: "sse",
  events,
});

export const responseStarted = (responseId: string = DEFAULT_RESPONSE_ID): SseEvent => ({
  type: "response.created",
  response: {
    id: responseId,
  },
});

export const assistantMessage = (text: string, itemId: string = DEFAULT_MESSAGE_ID): SseEvent => ({
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
});

export const responseCompleted = (responseId: string = DEFAULT_RESPONSE_ID): SseEvent => ({
  type: "response.completed",
  response: {
    id: responseId,
    usage: {
      input_tokens: 0,
      input_tokens_details: null,
      output_tokens: 0,
      output_tokens_details: null,
      total_tokens: 0,
    },
  },
});
