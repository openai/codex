import type { TranscriptionConfig } from "./config";

import { getApiKey, getBaseUrl, loadConfig } from "./config";
import { CLI_VERSION, ORIGIN, getSessionId } from "./session";
import { EventEmitter } from "events";
import { createRequire } from "node:module";
import OpenAI from "openai";
// workaround since pvrecorder-node is a commonjs module
import { OpenAIRealtimeWS } from "openai/beta/realtime/ws";

const require = createRequire(import.meta.url);
const { PvRecorder } = require("@picovoice/pvrecorder-node");

export interface TranscriptionEvent {
  type: string;
  delta?: string;
  transcript?: string;
}

export class RealtimeTranscriber extends EventEmitter {
  private rt: OpenAIRealtimeWS | null = null;
  private recorder: typeof PvRecorder | null = null;
  private isConnected = false;
  private isRecording = false;
  private transcriptionConfig: TranscriptionConfig;

  constructor() {
    super();
    // Load config and use it for defaults
    const config = loadConfig();

    // Load values from config with sensible defaults
    this.transcriptionConfig = {
      input_audio_transcription: config.transcription
        ?.input_audio_transcription || {
        model: "gpt-4o-transcribe",
        prompt: "",
        language: "en",
      },
      turn_detection: config.transcription?.turn_detection || {
        type: "server_vad",
        threshold: 0.6,
        prefix_padding_ms: 400,
        silence_duration_ms: 500,
      },
      input_audio_noise_reduction: config.transcription
        ?.input_audio_noise_reduction || {
        type: "near_field",
      },
    };

    this.setupSignalHandlers();
  }

  private setupSignalHandlers() {
    process.on("SIGINT", () => this.cleanup());
    process.on("SIGTERM", () => this.cleanup());
  }

  public async start(): Promise<void> {
    try {
      // Check API key
      const apiKey = getApiKey("openai");
      if (!apiKey) {
        throw new Error("OPENAI_API_KEY not found in environment variables");
      }

      // Initialize OpenAI client
      const client = new OpenAI({
        apiKey: apiKey,
        baseURL: getBaseUrl("openai"),
        defaultHeaders: {
          originator: ORIGIN,
          version: CLI_VERSION,
          session_id: getSessionId() || "",
        },
      });

      const model =
        this.transcriptionConfig.input_audio_transcription?.model ||
        "gpt-4o-transcribe";

      // Initialize the realtime client
      this.rt = new OpenAIRealtimeWS({ model }, client);

      // Set up event handlers
      this.rt.on("error", (error) => {
        this.emit("error", error);
      });

      this.rt.on(
        "conversation.item.input_audio_transcription.delta",
        (event) => {
          this.emit("transcription", {
            type: "transcription.delta",
            delta: event.delta,
          });
        },
      );

      this.rt.on(
        "conversation.item.input_audio_transcription.completed",
        (event) => {
          this.emit("transcription", {
            type: "transcription.done",
            transcript: event.transcript,
          });
        },
      );

      // Set up WebSocket connection
      this.rt.socket.on("open", () => {
        this.isConnected = true;
        this.emit("connected");

        // Configure the session
        this.rt?.send({
          type: "session.update",
          session: {
            input_audio_format: "pcm16",
            input_audio_transcription:
              this.transcriptionConfig.input_audio_transcription,
            turn_detection: this.transcriptionConfig.turn_detection,
            input_audio_noise_reduction:
              this.transcriptionConfig.input_audio_noise_reduction,
          },
        });

        // Start audio capture once WebSocket is connected
        this.startAudioCapture();
      });

      this.rt.socket.on("close", (code: number, reason: string) => {
        if (code !== 1000) {
          // 1000 is a normal close
          this.emit(
            "error",
            new Error(`WebSocket closed: code=${code}, reason=${reason}`),
          );
        }
        this.isConnected = false;
        this.emit("disconnected");
      });

      this.rt.socket.on("error", (error: Error) => {
        this.emit("error", error);
      });
    } catch (error) {
      this.emit("error", error);
      this.cleanup();
      throw error;
    }
  }

  private startAudioCapture() {
    try {
      // Get available audio devices
      const devices = PvRecorder.getAvailableDevices();
      if (devices.length === 0) {
        throw new Error("No audio input device found");
      }

      // Create recorder with first available device
      const frameLength = 512;
      this.recorder = new PvRecorder(frameLength, 0);

      // Start recording
      this.recorder.start();
      this.isRecording = true;

      // Process audio frames
      this.processAudioFrames();
    } catch (error) {
      this.emit("error", error);
      this.stopAudioCapture();
      throw error;
    }
  }

  private async processAudioFrames() {
    if (!this.recorder || !this.isRecording) {
      return;
    }

    try {
      while (this.isRecording && this.isConnected) {
        try {
          // We need to await each audio frame sequentially in the loop
          // to maintain proper audio stream ordering
          // eslint-disable-next-line no-await-in-loop
          const frame = await this.recorder.read();

          // Convert Int16Array to Buffer and send to OpenAI
          if (this.rt && this.isConnected) {
            const buffer = Buffer.from(frame.buffer);
            this.rt.send({
              type: "input_audio_buffer.append",
              audio: buffer.toString("base64"),
            });
          }
        } catch (error) {
          // Silently break out if it's an InvalidState error (happens when stopping)
          if (
            error instanceof Error &&
            (error.constructor.name === "PvRecorderStatusInvalidStateError" ||
              // require import doesn't perserve class prototype
              error.message?.includes("failed to read audio data frame"))
          ) {
            break;
          }
          // Re-throw other errors
          throw error;
        }
      }
    } catch (error) {
      this.emit("error", error);
    } finally {
      this.stopAudioCapture();
    }
  }

  private stopAudioCapture() {
    if (this.recorder) {
      try {
        this.recorder.stop();
        this.recorder.release();
      } catch (error) {
        this.emit("error", error);
      }
      this.recorder = null;
      this.isRecording = false;
    }
  }

  public cleanup(): void {
    this.stopAudioCapture();

    if (this.rt) {
      try {
        this.rt.close();
      } catch (error) {
        this.emit("error", error);
      }
      this.rt = null;
    }

    this.isConnected = false;
  }
}
