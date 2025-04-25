import { writeFileSync, readFileSync, existsSync, mkdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";
import { ADVERSYS_SESSIONS_DIR, ADVERSYS_TOOLS_DIR } from "../index";

// Generic interface for database storage
export interface Store<T> {
  get(id: string): T | null;
  getAll(): T[];
  save(item: T): void;
  delete(id: string): boolean;
}

/**
 * JSON-based storage compatible with Codex patterns
 */
export class JsonStore<T extends { id: string }> implements Store<T> {
  private items: T[] = [];
  private filePath: string;
  private loaded = false;

  constructor(fileName: string, baseDir: string) {
    // Determine storage directory, fallback to ~/.adversys if undefined
    const dir = typeof baseDir === 'string' && baseDir ? baseDir : join(homedir(), '.adversys');
    // Ensure storage directory exists
    if (!existsSync(dir)) {
      mkdirSync(dir, { recursive: true });
    }
    this.filePath = join(dir, fileName);
    this.load();
  }

  private load(): void {
    if (this.loaded) return;
    
    if (existsSync(this.filePath)) {
      try {
        const data = readFileSync(this.filePath, 'utf8');
        this.items = JSON.parse(data);
      } catch (error) {
        console.error(`Error loading data from ${this.filePath}:`, error);
        this.items = [];
      }
    } else {
      this.items = [];
    }
    
    this.loaded = true;
  }

  private persistData(): void {
    try {
      writeFileSync(this.filePath, JSON.stringify(this.items, null, 2), 'utf8');
    } catch (error) {
      console.error(`Error saving data to ${this.filePath}:`, error);
    }
  }

  get(id: string): T | null {
    this.load();
    const item = this.items.find(item => item.id === id);
    return item || null;
  }

  getAll(): T[] {
    this.load();
    return [...this.items];
  }

  save(item: T): void {
    this.load();
    const index = this.items.findIndex(i => i.id === item.id);
    
    if (index !== -1) {
      this.items[index] = item;
    } else {
      this.items.push(item);
    }
    
    this.persistData();
  }

  delete(id: string): boolean {
    this.load();
    const initialLength = this.items.length;
    this.items = this.items.filter(item => item.id !== id);
    
    if (this.items.length !== initialLength) {
      this.persistData();
      return true;
    }
    
    return false;
  }
}

// Create session store
export const createSessionStore = <T extends { id: string }>() => {
  return new JsonStore<T>('sessions.json', ADVERSYS_SESSIONS_DIR);
};

// Create tools store
export const createToolsStore = <T extends { id: string }>() => {
  return new JsonStore<T>('tools.json', ADVERSYS_TOOLS_DIR);
}; 