import { describe, it, expect, vi, beforeEach } from "vitest";
import { EventEmitter } from "events";

// Define types for our mocks
type MockSocket = {
  socket: EventEmitter;
  send: ReturnType<typeof vi.fn>;
  close: ReturnType<typeof vi.fn>;
  on: ReturnType<typeof vi.fn>;
  emit: ReturnType<typeof vi.fn>;
};

type MockRecorder = {
  start: ReturnType<typeof vi.fn>;
  read: ReturnType<typeof vi.fn>;
  stop: ReturnType<typeof vi.fn>;
  release: ReturnType<typeof vi.fn>;
};

// Store mocks in this object so we can access them in tests
const mocks: {
  webSocket: MockSocket | null;
  PvRecorder: MockRecorder | null;
} = {
  webSocket: null,
  PvRecorder: null,
};

// Mock dependencies before importing the module under test
vi.mock("openai/beta/realtime/ws", () => {
  return {
    OpenAIRealtimeWS: vi.fn().mockImplementation(() => {
      const eventHandlers: Record<string, (data: any) => void> = {};
      const socketMock: MockSocket = {
        socket: new EventEmitter(),
        send: vi.fn(),
        close: vi.fn(),
        on: vi.fn().mockImplementation((event, handler) => {
          eventHandlers[event] = handler;
        }),
        emit: vi.fn().mockImplementation((event, data) => {
          if (eventHandlers[event]) {
            eventHandlers[event](data);
          }
        }),
      };
      // Store reference in our mocks object for test access
      mocks.webSocket = socketMock;
      return socketMock;
    }),
  };
});

vi.mock("@picovoice/pvrecorder-node", () => {
  return {
    PvRecorder: vi.fn().mockImplementation(() => {
      const recorderMock: MockRecorder = {
        start: vi.fn(),
        read: vi.fn().mockReturnValue(new Int16Array([1, 2, 3])),
        stop: vi.fn(),
        release: vi.fn(),
      };
      // Store reference in our mocks object for test access
      mocks.PvRecorder = recorderMock;
      return recorderMock;
    }),
  };
});

vi.mock("../src/utils/config.js", () => ({
  getApiKey: vi.fn().mockReturnValue("fake-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.openai.com/v1"),
}));

vi.mock("../src/utils/session.js", () => ({
  CLI_VERSION: "test-version",
  ORIGIN: "test-origin",
  getSessionId: vi.fn().mockReturnValue("test-session-id"),
}));

// Import the module under test after mocks are defined
import { RealtimeTranscriber } from "../src/utils/transcriber.js";

describe("RealtimeTranscriber", () => {
  let transcriber;

  beforeEach(() => {
    vi.clearAllMocks();
    // Reset our references
    mocks.webSocket = null;
    mocks.PvRecorder = null;
  });

  it("creates instance without error", () => {
    transcriber = new RealtimeTranscriber();
    expect(transcriber).toBeDefined();
  });

  it("can start and stop", async () => {
    transcriber = new RealtimeTranscriber();
    await transcriber.start();
    transcriber.cleanup();
    expect(true).toBe(true); // Simple assertion just to verify no errors
  });

  it("emits transcription events when receiving data", async () => {
    const transcriber = new RealtimeTranscriber();
    const transcriptionHandler = vi.fn();
    transcriber.on("transcription", transcriptionHandler);

    await transcriber.start();

    // References should be populated after start() is called
    const ws = mocks.webSocket;
    expect(ws).toBeTruthy();

    // TypeScript needs non-null assertion since we've checked with expect
    // Simulate WebSocket open event
    ws!.socket.emit("open");

    // Simulate receiving transcription data
    ws!.emit("conversation.item.input_audio_transcription.delta", {
      delta: "Hello world",
    });

    expect(transcriptionHandler).toHaveBeenCalledWith({
      type: "transcription.delta",
      delta: "Hello world",
    });
  });

  it("emits errors when WebSocket encounters problems", async () => {
    const transcriber = new RealtimeTranscriber();
    const errorHandler = vi.fn();
    transcriber.on("error", errorHandler);

    await transcriber.start();

    const ws = mocks.webSocket;
    expect(ws).toBeTruthy();

    // Simulate WebSocket error
    const testError = new Error("WebSocket error");
    ws!.socket.emit("error", testError);

    expect(errorHandler).toHaveBeenCalledWith(testError);
  });

  it("starts audio recording when WebSocket connection opens", async () => {
    // Spy on the private startAudioCapture method
    const startAudioCaptureSpy = vi.spyOn(
      RealtimeTranscriber.prototype as any,
      "startAudioCapture",
    );

    // Create transcriber and start it
    const transcriber = new RealtimeTranscriber();
    await transcriber.start();

    const ws = mocks.webSocket;
    expect(ws).toBeTruthy();

    // Simulate WebSocket open event
    ws!.socket.emit("open");

    // Verify that startAudioCapture was called
    expect(startAudioCaptureSpy).toHaveBeenCalled();
  });

  it("sends session.update event when WebSocket connection opens", async () => {
    const transcriber = new RealtimeTranscriber({
      model: "test-model",
      language: "fr",
    });

    await transcriber.start();

    const ws = mocks.webSocket;
    expect(ws).toBeTruthy();

    // Simulate WebSocket open event
    ws!.socket.emit("open");

    // Verify that send was called with session.update event
    expect(ws!.send).toHaveBeenCalledWith({
      type: "session.update",
      session: {
        input_audio_format: "pcm16",
        input_audio_transcription: {
          model: "test-model",
          prompt: "",
          language: "fr",
        },
        turn_detection: {
          type: "server_vad",
          threshold: 0.5,
          prefix_padding_ms: 300,
          silence_duration_ms: 500,
        },
        input_audio_noise_reduction: {
          type: "near_field",
        },
      },
    });
  });
});
