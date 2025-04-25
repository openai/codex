/* eslint-disable no-console */

import type { FileContent } from "./context_files.js";

import path from "path";

/**
 * Builds file-size and total-size maps for the provided files, keyed by absolute path.
 *
 * @param root - The root directory (absolute path) to treat as the top-level. Ascension stops here.
 * @param files - An array of FileContent objects, each with a path and content.
 * @returns A tuple [fileSizeMap, totalSizeMap] where:
 *  - fileSizeMap[path] = size (in characters) of the file
 *  - totalSizeMap[path] = cumulative size (in characters) for path (file or directory)
 */
export function computeSizeMap(
  root: string,
  files: Array<FileContent>,
): [Record<string, number>, Record<string, number>] {
  const rootAbs = path.resolve(root);
  const fileSizeMap: Record<string, number> = {};
  const totalSizeMap: Record<string, number> = {};

  for (const fc of files) {
    const pAbs = path.resolve(fc.path);
    const length = fc.content.length;

    // Record size in fileSizeMap
    fileSizeMap[pAbs] = length;

    // Ascend from pAbs up to root, adding size along the way.
    let current = pAbs;

    // eslint-disable-next-line no-constant-condition
    while (true) {
      totalSizeMap[current] = (totalSizeMap[current] ?? 0) + length;
      if (current === rootAbs) {
        break;
      }

      const parent = path.dirname(current);
      // If we've reached the top or gone outside root, break.
      if (parent === current) {
        // e.g. we're at "/" in a *nix system or some root in Windows.
        break;
      }
      // If we have gone above the root (meaning the parent no longer starts with rootAbs), break.
      if (!parent.startsWith(rootAbs) && parent !== rootAbs) {
        break;
      }
      current = parent;
    }
  }

  return [fileSizeMap, totalSizeMap];
}

/**
 * Builds a mapping of directories to their immediate children. The keys and values
 * are absolute paths. For each path in totalSizeMap (except the root itself), we find
 * its parent (if also in totalSizeMap) and add the path to the children of that parent.
 *
 * @param root - The root directory (absolute path).
 * @param totalSizeMap - A map from path -> cumulative size.
 * @returns A record that maps directory paths to arrays of child paths.
 */
export function buildChildrenMap(
  root: string,
  totalSizeMap: Record<string, number>,
): Record<string, Array<string>> {
  const rootAbs = path.resolve(root);
  const childrenMap: Record<string, Array<string>> = {};

  // Initialize all potential keys so that each path has an entry.
  for (const p of Object.keys(totalSizeMap)) {
    if (!childrenMap[p]) {
      childrenMap[p] = [];
    }
  }

  for (const p of Object.keys(totalSizeMap)) {
    if (p === rootAbs) {
      continue;
    }
    const parent = path.dirname(p);

    // If the parent is also tracked in totalSizeMap, we record p as a child.
    if (totalSizeMap[parent] !== undefined && parent !== p) {
      if (!childrenMap[parent]) {
        childrenMap[parent] = [];
      }

      childrenMap[parent].push(p);
    }
  }

  // Sort the children.
  for (const val of Object.values(childrenMap)) {
    val.sort((a, b) => {
      return a.localeCompare(b);
    });
  }

  return childrenMap;
}

/**
 * Recursively prints a directory/file tree, showing size usage.
 *
 * @param current - The current absolute path (directory or file) to print.
 * @param childrenMap - A mapping from directory paths to an array of their child paths.
 * @param fileSizeMap - A map from file path to file size (characters).
 * @param totalSizeMap - A map from path to total cumulative size.
 * @param prefix - The current prefix used for ASCII indentation.
 * @param isLast - Whether the current path is the last child in its parent.
 * @param contextLimit - The context limit for reference.
 */
export function printSizeTree(
  current: string,
  childrenMap: Record<string, Array<string>>,
  fileSizeMap: Record<string, number>,
  totalSizeMap: Record<string, number>,
  prefix: string,
  isLast: boolean,
  contextLimit: number,
): void {
  const connector = isLast ? "└──" : "├──";
  const label = path.basename(current) || current;
  const totalSz = totalSizeMap[current] ?? 0;
  const percentageOfLimit =
    contextLimit > 0 ? (totalSz / contextLimit) * 100 : 0;

  if (fileSizeMap[current] !== undefined) {
    // It's a file
    const fileSz = fileSizeMap[current];
    console.log(
      `${prefix}${connector} ${label} [file: ${fileSz} bytes, cumulative: ${totalSz} bytes, ${percentageOfLimit.toFixed(
        2,
      )}% of limit]`,
    );
  } else {
    // It's a directory
    console.log(
      `${prefix}${connector} ${label} [dir: ${totalSz} bytes, ${percentageOfLimit.toFixed(
        2,
      )}% of limit]`,
    );
  }

  const newPrefix = prefix + (isLast ? "    " : "│   ");
  const children = childrenMap[current] || [];
  for (let i = 0; i < children.length; i++) {
    const child = children[i];
    const childIsLast = i === children.length - 1;
    printSizeTree(
      child!,
      childrenMap,
      fileSizeMap,
      totalSizeMap,
      newPrefix,
      childIsLast,
      contextLimit,
    );
  }
}

/**
 * Prints a size breakdown for the entire directory (and subpaths), listing cumulative percentages.
 *
 * @param directory - The directory path (absolute or relative) for which to print the breakdown.
 * @param files - The array of FileContent representing the files under that directory.
 * @param contextLimit - The maximum context character limit.
 */
export function printDirectorySizeBreakdown(
  directory: string,
  files: Array<FileContent>,
  contextLimit = 300_000,
): void {
  const rootAbs = path.resolve(directory);
  const [fileSizeMap, totalSizeMap] = computeSizeMap(rootAbs, files);
  const childrenMap = buildChildrenMap(rootAbs, totalSizeMap);

  console.log("\nContext size breakdown by directory and file:");

  const rootTotal = totalSizeMap[rootAbs] ?? 0;
  const rootPct =
    contextLimit > 0 ? ((rootTotal / contextLimit) * 100).toFixed(2) : "0";

  const rootLabel = path.basename(rootAbs) || rootAbs;
  console.log(`${rootLabel} [dir: ${rootTotal} bytes, ${rootPct}% of limit]`);

  const rootChildren = childrenMap[rootAbs] || [];
  rootChildren.sort((a, b) => a.localeCompare(b));

  for (let i = 0; i < rootChildren.length; i++) {
    const child = rootChildren[i];
    const childIsLast = i === rootChildren.length - 1;
    printSizeTree(
      child!,
      childrenMap,
      fileSizeMap,
      totalSizeMap,
      "",
      childIsLast,
      contextLimit,
    );
  }
}
