export type ApplyPatchCreateFileOp = {
  type: "create";
  path: string;
  content: string;
};

export type ApplyPatchDeleteFileOp = {
  type: "delete";
  path: string;
};

export type ApplyPatchUpdateFileOp = {
  type: "update";
  path: string;
  update: string;
  added: number;
  deleted: number;
};

export type ApplyPatchOp =
  | ApplyPatchCreateFileOp
  | ApplyPatchDeleteFileOp
  | ApplyPatchUpdateFileOp;

const PATCH_PREFIX = "*** Begin Patch\n";
const PATCH_SUFFIX = "\n*** End Patch";
const ADD_FILE_PREFIX = "*** Add File: ";
const DELETE_FILE_PREFIX = "*** Delete File: ";
const UPDATE_FILE_PREFIX = "*** Update File: ";
const END_OF_FILE_PREFIX = "*** End of File";
const HUNK_ADD_LINE_PREFIX = "+";

/**
 * @returns null when the patch is invalid
 */
export function parseApplyPatch(patch: string): Array<ApplyPatchOp> | null {
  if (!patch.startsWith(PATCH_PREFIX)) {
    // Patch must begin with '*** Begin Patch'
    return null;
  } else if (!patch.endsWith(PATCH_SUFFIX)) {
    // Patch must end with '*** End Patch'
    return null;
  }

  const patchBody = patch.slice(
    PATCH_PREFIX.length,
    patch.length - PATCH_SUFFIX.length,
  );

  const lines = patchBody.split("\n");

  const ops: Array<ApplyPatchOp> = [];

  for (const line of lines) {
    if (line.startsWith(END_OF_FILE_PREFIX)) {
      continue;
    } else if (line.startsWith(ADD_FILE_PREFIX)) {
      ops.push({
        type: "create",
        path: line.slice(ADD_FILE_PREFIX.length).trim(),
        content: "",
      });
      continue;
    } else if (line.startsWith(DELETE_FILE_PREFIX)) {
      ops.push({
        type: "delete",
        path: line.slice(DELETE_FILE_PREFIX.length).trim(),
      });
      continue;
    } else if (line.startsWith(UPDATE_FILE_PREFIX)) {
      ops.push({
        type: "update",
        path: line.slice(UPDATE_FILE_PREFIX.length).trim(),
        update: "",
        added: 0,
        deleted: 0,
      });
      continue;
    }

    const lastOp = ops[ops.length - 1];

    if (lastOp?.type === "create") {
      lastOp.content = appendLine(
        lastOp.content,
        line.slice(HUNK_ADD_LINE_PREFIX.length),
      );
      continue;
    }

    if (lastOp?.type !== "update") {
      // Expected update op but got ${lastOp?.type} for line ${line}
      return null;
    }

    if (line.startsWith(HUNK_ADD_LINE_PREFIX)) {
      lastOp.added += 1;
    } else if (line.startsWith("-")) {
      lastOp.deleted += 1;
    }
    lastOp.update += lastOp.update ? "\n" + line : line;
  }

  return ops;
}

function appendLine(content: string, line: string) {
  if (!content.length) {
    return line;
  }
  return [content, line].join("\n");
}
