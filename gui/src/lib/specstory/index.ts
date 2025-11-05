import { Conversation, Message } from '../types';

export interface SpecStoryConfig {
  apiKey?: string;
  baseUrl?: string;
  enableAutoSave: boolean;
  saveInterval: number; // milliseconds
}

export interface SpecStoryEntry {
  id: string;
  type: 'conversation' | 'message' | 'command';
  timestamp: string;
  data: any;
  metadata?: Record<string, any>;
}

class SpecStoryManager {
  private config: SpecStoryConfig;
  private queue: SpecStoryEntry[] = [];
  private saveTimer?: NodeJS.Timeout;
  private isInitialized = false;

  constructor(config: Partial<SpecStoryConfig> = {}) {
    this.config = {
      enableAutoSave: true,
      saveInterval: 30000, // 30 seconds
      ...config,
    };
  }

  async initialize(): Promise<void> {
    if (this.isInitialized) return;

    try {
      // Load existing data from localStorage
      const savedData = localStorage.getItem('specstory-queue');
      if (savedData) {
        this.queue = JSON.parse(savedData);
      }

      if (this.config.enableAutoSave) {
        this.startAutoSave();
      }

      this.isInitialized = true;
      console.log('SpecStory initialized');
    } catch (error) {
      console.error('Failed to initialize SpecStory:', error);
    }
  }

  private startAutoSave(): void {
    if (this.saveTimer) {
      clearInterval(this.saveTimer);
    }

    this.saveTimer = setInterval(() => {
      this.saveToStorage();
    }, this.config.saveInterval);
  }

  private saveToStorage(): void {
    try {
      localStorage.setItem('specstory-queue', JSON.stringify(this.queue));
      console.log(`SpecStory: Saved ${this.queue.length} entries`);
    } catch (error) {
      console.error('Failed to save SpecStory data:', error);
    }
  }

  async saveConversation(conversation: Conversation): Promise<void> {
    const entry: SpecStoryEntry = {
      id: `conv_${conversation.id}`,
      type: 'conversation',
      timestamp: new Date().toISOString(),
      data: conversation,
      metadata: {
        model: conversation.model,
        messageCount: conversation.messageCount,
        duration: Date.now() - new Date(conversation.createdAt).getTime(),
      },
    };

    this.queue.push(entry);

    if (this.config.enableAutoSave) {
      this.saveToStorage();
    }

    // Send to remote service if configured
    if (this.config.apiKey && this.config.baseUrl) {
      await this.sendToRemote(entry);
    }
  }

  async saveMessage(conversationId: string, message: Message): Promise<void> {
    const entry: SpecStoryEntry = {
      id: `msg_${conversationId}_${Date.now()}`,
      type: 'message',
      timestamp: new Date().toISOString(),
      data: {
        conversationId,
        message,
      },
      metadata: {
        role: message.role,
        contentLength: message.content.length,
      },
    };

    this.queue.push(entry);

    if (this.config.enableAutoSave) {
      this.saveToStorage();
    }

    if (this.config.apiKey && this.config.baseUrl) {
      await this.sendToRemote(entry);
    }
  }

  async saveCommand(command: string, args: string[], result: any): Promise<void> {
    const entry: SpecStoryEntry = {
      id: `cmd_${Date.now()}`,
      type: 'command',
      timestamp: new Date().toISOString(),
      data: {
        command,
        args,
        result,
      },
      metadata: {
        success: result.success,
        executionTime: result.executionTime,
      },
    };

    this.queue.push(entry);

    if (this.config.enableAutoSave) {
      this.saveToStorage();
    }

    if (this.config.apiKey && this.config.baseUrl) {
      await this.sendToRemote(entry);
    }
  }

  private async sendToRemote(entry: SpecStoryEntry): Promise<void> {
    try {
      const response = await fetch(`${this.config.baseUrl}/api/specstory`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${this.config.apiKey}`,
        },
        body: JSON.stringify(entry),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      console.log('SpecStory: Sent entry to remote service');
    } catch (error) {
      console.error('Failed to send to SpecStory remote service:', error);
    }
  }

  async getEntries(type?: string, limit = 50): Promise<SpecStoryEntry[]> {
    let filtered = this.queue;

    if (type) {
      filtered = this.queue.filter(entry => entry.type === type);
    }

    // Sort by timestamp (newest first)
    filtered.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    return filtered.slice(0, limit);
  }

  async getConversations(): Promise<Conversation[]> {
    const conversationEntries = await this.getEntries('conversation');
    return conversationEntries.map(entry => entry.data);
  }

  async getMessages(conversationId: string): Promise<Message[]> {
    const messageEntries = this.queue.filter(
      entry => entry.type === 'message' && entry.data.conversationId === conversationId
    );

    messageEntries.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

    return messageEntries.map(entry => entry.data.message);
  }

  async exportData(): Promise<string> {
    const data = {
      config: this.config,
      entries: this.queue,
      exportedAt: new Date().toISOString(),
      version: '1.0.0',
    };

    return JSON.stringify(data, null, 2);
  }

  async importData(jsonData: string): Promise<void> {
    try {
      const data = JSON.parse(jsonData);

      if (data.entries && Array.isArray(data.entries)) {
        this.queue = [...this.queue, ...data.entries];
        this.saveToStorage();
        console.log(`SpecStory: Imported ${data.entries.length} entries`);
      }
    } catch (error) {
      console.error('Failed to import SpecStory data:', error);
      throw error;
    }
  }

  async clearData(): Promise<void> {
    this.queue = [];
    localStorage.removeItem('specstory-queue');
    console.log('SpecStory: Cleared all data');
  }

  getQueueLength(): number {
    return this.queue.length;
  }

  destroy(): void {
    if (this.saveTimer) {
      clearInterval(this.saveTimer);
      this.saveTimer = undefined;
    }
    this.saveToStorage();
  }
}

// Singleton instance
let specStoryInstance: SpecStoryManager | null = null;

export function getSpecStory(config?: Partial<SpecStoryConfig>): SpecStoryManager {
  if (!specStoryInstance) {
    specStoryInstance = new SpecStoryManager(config);
  }
  return specStoryInstance;
}

export { SpecStoryManager };
export default getSpecStory;
