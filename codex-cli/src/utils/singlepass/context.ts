/** Represents file contents with a path and its full text. */
export interface FileContent {
  path: string;
  content: string;
}

/**
 * Represents the context for a task, including:
 * - A prompt (the user's request)
 * - A list of input paths being considered editable
 * - A directory structure overview
 * - A collection of file contents
 */
export interface TaskContext {
  prompt: string;
  input_paths: Array<string>;
  input_paths_structure: string;
  files: Array<FileContent>;
}

/**
 * Renders a string version of the TaskContext, including a note about important output requirements,
 * summary of the directory structure (unless omitted), and an XML-like listing of the file contents.
 *
 * The user is instructed to produce only changes for files strictly under the specified paths
 * and provide full file contents in any modifications.
 */
export function renderTaskContext(taskContext: TaskContext): string {
  const inputPathsJoined = taskContext.input_paths.join(", ");
  return `
  Complete the following task: ${taskContext.prompt}
  
  # IMPORTANT OUTPUT REQUIREMENTS
  - UNDER NO CIRCUMSTANCES PRODUCE PARTIAL OR TRUNCATED FILE CONTENT. You MUST provide the FULL AND FINAL content for every file modified.
  - ALWAYS INCLUDE THE COMPLETE UPDATED VERSION OF THE FILE, do not omit or only partially include lines.
  - ONLY produce changes for files located strictly under ${inputPathsJoined}.
  - ALWAYS produce absolute paths in the output.
  - Do not delete or change code UNRELATED to the task.
  
  # **Directory structure**
  ${taskContext.input_paths_structure}
  
  # Files
  ${renderFilesToXml(taskContext.files)}
   `;
}

/**
 * Converts the provided list of FileContent objects into a custom XML-like format.
 *
 * For each file, we embed the content in a CDATA section.
 */
function renderFilesToXml(files: Array<FileContent>): string {
  const fileContents = files
    .map(
      (fc) => `
      <file>
        <path>${fc.path}</path>
        <content><![CDATA[${fc.content}]]></content>
      </file>`,
    )
    .join("");

  return `<files>\n${fileContents}\n</files>`;
}
