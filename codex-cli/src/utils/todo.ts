import fs from "fs";
import os from "os";
import path from "path";

const todoFile = path.join(os.homedir(), ".codex", "todo.json");

export function loadTodos(): Array<string> {
  try {
    if (fs.existsSync(todoFile)) {
      return JSON.parse(fs.readFileSync(todoFile, "utf-8"));
    }
  } catch (e) {
    // ignore errors
  }
  return [];
}
export function saveTodos(todos: Array<string>): void {
  fs.mkdirSync(path.dirname(todoFile), { recursive: true });
  fs.writeFileSync(todoFile, JSON.stringify(todos, null, 2));
}
