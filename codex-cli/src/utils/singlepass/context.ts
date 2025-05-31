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
 * - Information about hidden files
 */
export interface TaskContext {
  prompt: string;
  input_paths: Array<string>;
  input_paths_structure: string;
  files: Array<FileContent>;
  hiddenFileInfo?: {
    count: number;
    examples: string[];
    userSpecified: boolean;
  };
}

/**
 * Renders a string version of the TaskContext, including a note about important output requirements,
 * summary of the directory structure, and information about hidden files if applicable.
 */
export function renderTaskContext(taskContext: TaskContext): string {
  // Generate hidden files notice if applicable
  let hiddenFilesNotice = "";
  if (taskContext.hiddenFileInfo && taskContext.hiddenFileInfo.count > 0) {
    hiddenFilesNotice = `
    # IMPORTANT SECURITY RESTRICTIONS
    - ${taskContext.hiddenFileInfo.count} files are hidden from your view
    - Examples include: ${taskContext.hiddenFileInfo.examples.join(", ")}
    - YOU CANNOT ACCESS THESE FILES under any circumstances
    - DO NOT suggest viewing the contents of these files
    - DO NOT make recommendations that depend on hidden content
    - DO NOT ask the user to reveal content from these files
    `;
  }

  return `
  Complete the following task: ${taskContext.prompt}
  
  ${hiddenFilesNotice}
  
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
