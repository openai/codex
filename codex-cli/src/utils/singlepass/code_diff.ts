import type { EditedFiles, FileOperation } from "./file_ops";

import { createTwoFilesPatch } from "diff";

/**************************************
 * ANSI color codes for output styling
 **************************************/
const RED = "\u001b[31m";
const GREEN = "\u001b[32m";
const CYAN = "\u001b[36m";
const YELLOW = "\u001b[33m";
const RESET = "\u001b[0m";

/******************************************************
 * Generate a unified diff of two file contents
 *  akin to generate_file_diff(original, updated)
 ******************************************************/
export function generateFileDiff(
  originalContent: string,
  updatedContent: string,
  filePath: string,
): string {
  return createTwoFilesPatch(
    `${filePath} (original)`,
    `${filePath} (modified)`,
    originalContent,
    updatedContent,
    undefined,
    undefined,
    { context: 5 },
  );
}

/******************************************************
 * Apply colorization to a unified diff
 * akin to generate_colored_diff(diff_content)
 ******************************************************/
export function generateColoredDiff(diffContent: string): string {
  const lines = diffContent.split(/\r?\n/);
  const coloredLines: Array<string> = [];

  for (const line of lines) {
    if (line.startsWith("+++") || line.startsWith("---")) {
      // keep these lines uncolored, preserving the original style
      coloredLines.push(line);
    } else if (line.startsWith("+")) {
      // color lines that begin with + but not +++
      coloredLines.push(`${GREEN}${line}${RESET}`);
    } else if (line.startsWith("-")) {
      // color lines that begin with - but not ---
      coloredLines.push(`${RED}${line}${RESET}`);
    } else if (line.startsWith("@@")) {
      // hunk header
      coloredLines.push(`${CYAN}${line}${RESET}`);
    } else {
      coloredLines.push(line);
    }
  }

  return coloredLines.join("\n");
}

/******************************************************
 * Count lines added and removed in a unified diff.
 * akin to generate_diff_stats(diff_content).
 ******************************************************/
export function generateDiffStats(diffContent: string): [number, number] {
  let linesAdded = 0;
  let linesRemoved = 0;

  const lines = diffContent.split(/\r?\n/);
  for (const line of lines) {
    if (line.startsWith("+") && !line.startsWith("+++")) {
      linesAdded += 1;
    } else if (line.startsWith("-") && !line.startsWith("---")) {
      linesRemoved += 1;
    }
  }

  return [linesAdded, linesRemoved];
}

/************************************************
 * Helper for generating a short header block
 ************************************************/
function generateDiffHeader(fileOp: FileOperation): string {
  const TTY_WIDTH = 80;
  const separatorLine = "=".repeat(TTY_WIDTH) + "\n";
  const subSeparatorLine = "-".repeat(TTY_WIDTH) + "\n";
  const headerLine = `Changes for: ${fileOp.path}`;
  return separatorLine + headerLine + "\n" + subSeparatorLine;
}

/****************************************************************
 * Summarize diffs for each file operation that has differences.
 * akin to generate_diff_summary(edited_files, original_files)
 ****************************************************************/
export function generateDiffSummary(
  editedFiles: EditedFiles,
  originalFileContents: Record<string, string>,
): [string, Array<FileOperation>] {
  let combinedDiffs = "";
  const opsToApply: Array<FileOperation> = [];

  for (const fileOp of editedFiles.ops) {
    const diffHeader = generateDiffHeader(fileOp);

    if (fileOp.delete) {
      // file will be deleted
      combinedDiffs += diffHeader + "File will be deleted.\n\n";
      opsToApply.push(fileOp);
      continue;
    } else if (fileOp.move_to) {
      combinedDiffs +=
        diffHeader + `File will be moved to: ${fileOp.move_to}\n\n`;
      opsToApply.push(fileOp);
      continue;
    }

    // otherwise it's an update
    const originalContent = originalFileContents[fileOp.path] ?? "";
    const updatedContent = fileOp.updated_full_content ?? "";

    if (originalContent === updatedContent) {
      // no changes => skip
      continue;
    }

    const diffOutput = generateFileDiff(
      originalContent,
      updatedContent,
      fileOp.path,
    );
    if (diffOutput.trim()) {
      const coloredDiff = generateColoredDiff(diffOutput);
      combinedDiffs += diffHeader + coloredDiff + "\n";
      opsToApply.push(fileOp);
    }
  }

  return [combinedDiffs, opsToApply];
}

/****************************************************************
 * Generate a user-friendly summary of the pending file ops.
 * akin to generate_edit_summary(ops_to_apply, original_files)
 ****************************************************************/
export function generateEditSummary(
  opsToApply: Array<FileOperation>,
  originalFileContents: Record<string, string>,
): string {
  if (!opsToApply || opsToApply.length === 0) {
    return "No changes detected.";
  }

  const summaryLines: Array<string> = [];
  for (const fileOp of opsToApply) {
    if (fileOp.delete) {
      // red for deleted
      summaryLines.push(`${RED}  Deleted: ${fileOp.path}${RESET}`);
    } else if (fileOp.move_to) {
      // yellow for moved
      summaryLines.push(
        `${YELLOW}  Moved: ${fileOp.path} -> ${fileOp.move_to}${RESET}`,
      );
    } else {
      const originalContent = originalFileContents[fileOp.path];
      const updatedContent = fileOp.updated_full_content ?? "";
      if (originalContent === undefined) {
        // newly created file
        const linesAdded = updatedContent.split(/\r?\n/).length;
        summaryLines.push(
          `${GREEN}  Created: ${fileOp.path} (+${linesAdded} lines)${RESET}`,
        );
      } else {
        const diffOutput = generateFileDiff(
          originalContent,
          updatedContent,
          fileOp.path,
        );
        const [added, removed] = generateDiffStats(diffOutput);
        summaryLines.push(
          `  Modified: ${fileOp.path} (${GREEN}+${added}${RESET}/${RED}-${removed}${RESET})`,
        );
      }
    }
  }

  return summaryLines.join("\n");
}
