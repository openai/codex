import { OpenAIRealtimeWS } from "openai/beta/realtime/ws";
import { EventEmitter } from "events";
import OpenAI from "openai";
// workaround since pvrecorder-node is a commonjs module
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { PvRecorder } = require("@picovoice/pvrecorder-node");

// API configuration
const API_KEY = process.env["OPENAI_API_KEY"];

export interface TranscriptionEvent {
  type: string;
  delta?: string;
  transcript?: string;
}

export interface TranscriberOptions {
  model?: string;
  language?: string;
}

export class RealtimeTranscriber extends EventEmitter {
  private rt: OpenAIRealtimeWS | null = null;
  private recorder: typeof PvRecorder | null = null;
  private isConnected = false;
  private isRecording = false;
  private model: string;
  private language: string;

  constructor(options: TranscriberOptions = {}) {
    super();
    this.model = options.model || "gpt-4o-mini-transcribe";
    this.language = options.language || "en";
    this.setupSignalHandlers();
  }

  private setupSignalHandlers() {
    process.on("SIGINT", () => this.cleanup());
    process.on("SIGTERM", () => this.cleanup());
  }

  public async start() {
    try {
      // Check API key
      if (!API_KEY) {
        throw new Error("OPENAI_API_KEY not found in environment variables");
      }

      // Initialize OpenAI client
      const client = new OpenAI({ apiKey: API_KEY });

      // Initialize the realtime client
      this.rt = new OpenAIRealtimeWS({ model: this.model }, client);

      // Set up event handlers
      this.rt.on("error", (error) => {
        console.error("OpenAI WebSocket error:", error);
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
            input_audio_transcription: {
              model: this.model,
              prompt: "",
              language: this.language,
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

        // Start audio capture once WebSocket is connected
        this.startAudioCapture();
      });

      this.rt.socket.on("close", (code: number, reason: string) => {
        if (code !== 1000) {
          // 1000 is a normal close
          console.error(`WebSocket closed: code=${code}, reason=${reason}`);
        }
        this.isConnected = false;
        this.emit("disconnected");
      });

      this.rt.socket.on("error", (error: Error) => {
        console.error("WebSocket error:", error);
      });
    } catch (error) {
      console.error("Failed to start transcription:", error);
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
      console.error("Failed to start audio capture:", error);
      this.stopAudioCapture();
      throw error;
    }
  }

  private async processAudioFrames() {
    if (!this.recorder || !this.isRecording) return;

    try {
      while (this.isRecording && this.isConnected) {
        try {
          const frame = await this.recorder.read();

          // Convert Int16Array to Buffer and send to OpenAI
          if (this.rt && this.isConnected) {
            const buffer = Buffer.from(frame.buffer);
            this.rt.send({
              type: "input_audio_buffer.append",
              audio: buffer.toString("base64"),
            });
          }
        } catch (error: any) {
          // Silently break out if it's an InvalidState error (happens when stopping)
          if (
            error.constructor.name === "PvRecorderStatusInvalidStateError" ||
            error.message?.includes("failed to read audio data frame")
          ) {
            break;
          }
          // Re-throw other errors
          throw error;
        }
      }
    } catch (error) {
      console.error("Error processing audio frames:", error);
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
        console.error("Error while stopping audio recording:", error);
      }
      this.recorder = null;
      this.isRecording = false;
    }
  }

  public cleanup() {
    this.stopAudioCapture();

    if (this.rt) {
      try {
        this.rt.close();
      } catch (error) {
        console.error("Error closing WebSocket connection:", error);
      }
      this.rt = null;
    }

    this.isConnected = false;
  }
}
