import { readdirSync, statSync } from "fs";
import * as path from "path";

export interface Config {
  labels: Record<string, LabelConfig>;
}

export interface LabelConfig {
  /** Returns the prompt template. */
  getPromptTemplate(): string;
}
