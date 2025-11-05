/**
 * Transport abstraction for Orchestrator RPC
 * Supports TCP (127.0.0.1) with auto-detection
 */

import EventEmitter from 'eventemitter3';

export type TransportPreference = 'auto' | 'tcp';

export interface TransportConfig {
  preference: TransportPreference;
  tcpHost?: string;
  tcpPort?: number;
}

export interface TransportEvents {
  message: (data: Uint8Array) => void;
  error: (error: Error) => void;
  close: () => void;
}

export abstract class Transport extends EventEmitter<TransportEvents> {
  abstract connect(): Promise<void>;
  abstract send(data: Uint8Array): Promise<void>;
  abstract close(): Promise<void>;
  abstract isConnected(): boolean;
}

/**
 * TCP Transport (127.0.0.1 only)
 * Uses WebSocket for browser compatibility
 */
export class TcpTransport extends Transport {
  private ws: WebSocket | null = null;
  private host: string;
  private port: number;

  constructor(host: string = '127.0.0.1', port: number = 0) {
    super();
    this.host = host;
    this.port = port;
  }

  async connect(): Promise<void> {
    // If port is 0, read from .codex/orchestrator.port
    if (this.port === 0) {
      this.port = await this.readPortFromFile();
    }

    return new Promise((resolve, reject) => {
      try {
        // Use WebSocket for TCP connection (requires ws:// server)
        this.ws = new WebSocket(`ws://${this.host}:${this.port}`);
        
        this.ws.binaryType = 'arraybuffer';

        this.ws.onopen = () => {
          resolve();
        };

        this.ws.onmessage = (event) => {
          const data = new Uint8Array(event.data as ArrayBuffer);
          this.emit('message', data);
        };

        this.ws.onerror = (event) => {
          const error = new Error('WebSocket error');
          this.emit('error', error);
          reject(error);
        };

        this.ws.onclose = () => {
          this.emit('close');
        };
      } catch (error) {
        reject(error);
      }
    });
  }

  async send(data: Uint8Array): Promise<void> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('Transport not connected');
    }

    this.ws.send(data);
  }

  async close(): Promise<void> {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  private async readPortFromFile(): Promise<number> {
    // Browser environment: try to read from known endpoint
    // For now, use default port 38247 (can be configured)
    return 38247;
  }
}

/**
 * Create transport based on preference
 */
export async function createTransport(config: TransportConfig): Promise<Transport> {
  const { preference, tcpHost, tcpPort } = config;

  switch (preference) {
    case 'auto':
    case 'tcp':
      return new TcpTransport(tcpHost, tcpPort);
    default:
      throw new Error(`Unknown transport preference: ${preference}`);
  }
}

