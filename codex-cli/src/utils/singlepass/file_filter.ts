import * as path from "path";
import * as fs from "fs";
import { minimatch } from "minimatch";
import { parsePromptForHiddenFiles } from "./prompt_parser";

export interface FileFilterResult {
  visibleFiles: string[];
  hiddenFileInfo: {
    count: number;
    examples: string[];
    userSpecified: boolean;
  };
}

export class FileFilter {
  private workspacePath: string;
  private agentIgnorePatterns: string[] = [];
  private promptPatterns: string[] = [];

  constructor(workspacePath: string, userPrompt: string) {
    this.workspacePath = workspacePath;
    this.loadAgentIgnorePatterns();

    // Parse prompt for hidden file patterns
    const parsedPrompt = parsePromptForHiddenFiles(userPrompt);
    this.promptPatterns = parsedPrompt.hiddenPatterns;
  }

  /**
   * Load patterns from .agentignore file if it exists
   */
  private loadAgentIgnorePatterns(): void {
    const agentIgnorePath = path.join(this.workspacePath, ".agentignore");
    if (fs.existsSync(agentIgnorePath)) {
      try {
        const content = fs.readFileSync(agentIgnorePath, "utf-8");
        this.agentIgnorePatterns = content
          .split("\n")
          .map((line) => line.trim())
          .filter((line) => line && !line.startsWith("#"));
      } catch {
        console.error(`Error reading .agentignore file: ${agentIgnorePath}`);
      }
    }
  }

  /**
   * Check if a file matches any of the hidden patterns
   */
  public isHidden(filePath: string): boolean {
    const relativePath = path.relative(this.workspacePath, filePath);

    // Apply .agentignore patterns in order, supporting negation
    let hidden = false;
    for (const pattern of this.agentIgnorePatterns) {
      try {
        if (pattern.startsWith("!")) {
          // Negated pattern: unhide if matches
          if (minimatch(relativePath, pattern.slice(1), { dot: true })) {
            hidden = false;
          }
        } else if (pattern.endsWith("/")) {
          if (
            relativePath === pattern.slice(0, -1) ||
            relativePath.startsWith(pattern)
          ) {
            hidden = true;
          }
        } else {
          // Normal pattern: hide if matches
          if (minimatch(relativePath, pattern, { dot: true })) {
            hidden = true;
          }
        }
      } catch (err) {
        continue;
      }
    }
    // Check against prompt patterns (these always hide)
    const matchesPrompt = this.promptPatterns.some((pattern) =>
      minimatch(relativePath, pattern, { dot: true }),
    );

    return hidden || matchesPrompt;
  }

  /**
   * Filter a list of files based on hidden patterns
   */
  public filterFiles(files: string[]): FileFilterResult {
    const visibleFiles: string[] = [];
    const hiddenFiles: string[] = [];

    for (const file of files) {
      if (this.isHidden(file)) {
        hiddenFiles.push(file);
      } else {
        visibleFiles.push(file);
      }
    }

    // Limit the number of examples to 5
    const examples = hiddenFiles.slice(0, 5).map((file) => path.basename(file));

    return {
      visibleFiles,
      hiddenFileInfo: {
        count: hiddenFiles.length,
        examples,
        userSpecified: this.promptPatterns.length > 0,
      },
    };
  }

  /**
   * Get the cleaned prompt with hidden directives removed
   */
  public getCleanedPrompt(originalPrompt: string): string {
    return parsePromptForHiddenFiles(originalPrompt).cleanedPrompt;
  }
}
