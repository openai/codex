import http from 'node:http';

const DEFAULT_RESPONSE_ID = 'resp_mock';
const DEFAULT_MESSAGE_ID = 'msg_mock';

type SseEvent = {
  type: string;
  [key: string]: unknown;
};

type SseResponseBody = {
  kind: 'sse';
  events: SseEvent[];
};

export type ResponsesProxyOptions = {
  responseBody: SseResponseBody;
  statusCode?: number;
};

export type ResponsesProxy = {
  url: string;
  close: () => Promise<void>;
};

const formatSseEvent = (event: SseEvent): string => {
  return `event: ${event.type}\n` + `data: ${JSON.stringify(event)}\n\n`;
};

export async function startResponsesTestProxy(
  options: ResponsesProxyOptions,
): Promise<ResponsesProxy> {
  const server = http.createServer((req, res) => {
    if (req.method === 'POST' && req.url === '/responses') {
      const status = options.statusCode ?? 200;
      res.statusCode = status;
      res.setHeader('content-type', 'text/event-stream');
      for (const event of options.responseBody.events) {
        res.write(formatSseEvent(event));
      }
      res.end();
      return;
    }
 
    res.statusCode = 404;
    res.end();
  });

  const url = await new Promise<string>((resolve, reject) => {
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      if (!address || typeof address === 'string') {
        reject(new Error('Unable to determine proxy address'));
        return;
      }
      server.off('error', reject);
      const info = address;
      resolve(`http://${info.address}:${info.port}`);
    });
    server.once('error', reject);
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
  return { url, close };
}

export const sse = (...events: SseEvent[]): SseResponseBody => ({
  kind: 'sse',
  events,
});

export const responseStarted = (responseId: string = DEFAULT_RESPONSE_ID): SseEvent => ({
  type: 'response.created',
  response: {
    id: responseId,
  },
});

export const assistantMessage = (text: string, itemId: string = DEFAULT_MESSAGE_ID): SseEvent => ({
  type: 'response.output_item.done',
  item: {
    type: 'message',
    role: 'assistant',
    id: itemId,
    content: [
      {
        type: 'output_text',
        text,
      },
    ],
  },
});

export const responseCompleted = (responseId: string = DEFAULT_RESPONSE_ID): SseEvent => ({
  type: 'response.completed',
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
