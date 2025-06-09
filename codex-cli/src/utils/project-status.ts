import fs from "fs/promises";
import path from "path";

export interface FileStats {
  extension: string;
  count: number;
  totalLines: number;
}

export interface ProjectStats {
  totalFiles: number;
  totalLines: number;
  filesByExtension: Record<string, FileStats>;
  recentFiles: Array<{
    path: string;
    lastModified: Date;
    lines: number;
  }>;
  projectSize: number; // in bytes
}

// 要忽略的目录和文件
const IGNORE_PATTERNS = [
  "node_modules",
  ".git",
  ".next",
  "dist",
  "build",
  ".codex",
  ".env",
  "package-lock.json",
  "yarn.lock",
  ".DS_Store",
];

// 代码文件扩展名
const CODE_EXTENSIONS = new Set([
  ".js",
  ".jsx",
  ".ts",
  ".tsx",
  ".py",
  ".java",
  ".cpp",
  ".c",
  ".h",
  ".cs",
  ".php",
  ".rb",
  ".go",
  ".rs",
  ".swift",
  ".kt",
  ".scala",
  ".html",
  ".css",
  ".scss",
  ".less",
  ".vue",
  ".svelte",
  ".json",
  ".yaml",
  ".yml",
  ".xml",
  ".sql",
  ".sh",
  ".bash",
  ".zsh",
]);

export async function analyzeProject(
  rootPath: string = process.cwd(),
): Promise<ProjectStats> {
  const stats: ProjectStats = {
    totalFiles: 0,
    totalLines: 0,
    filesByExtension: {},
    recentFiles: [],
    projectSize: 0,
  };

  async function analyzeDirectory(dirPath: string): Promise<void> {
    try {
      const entries = await fs.readdir(dirPath, { withFileTypes: true });

      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name);
        const relativePath = path.relative(rootPath, fullPath);

        // 跳过忽略的文件和目录
        if (
          IGNORE_PATTERNS.some(
            (pattern) =>
              relativePath.includes(pattern) || entry.name.startsWith("."),
          )
        ) {
          continue;
        }

        if (entry.isDirectory()) {
          await analyzeDirectory(fullPath);
        } else if (entry.isFile()) {
          await analyzeFile(fullPath, relativePath);
        }
      }
    } catch (error) {
      // 忽略无法访问的目录
      console.warn(`Cannot access directory: ${dirPath}`);
    }
  }

  async function analyzeFile(
    filePath: string,
    relativePath: string,
  ): Promise<void> {
    try {
      const fileStat = await fs.stat(filePath);
      const extension = path.extname(filePath).toLowerCase() || "no-extension";

      stats.totalFiles++;
      stats.projectSize += fileStat.size;

      // 初始化扩展名统计
      if (!stats.filesByExtension[extension]) {
        stats.filesByExtension[extension] = {
          extension,
          count: 0,
          totalLines: 0,
        };
      }

      stats.filesByExtension[extension].count++;

      // 只对代码文件计算行数
      let lineCount = 0;
      if (CODE_EXTENSIONS.has(extension)) {
        try {
          const content = await fs.readFile(filePath, "utf8");
          lineCount = content.split("\n").length;
          stats.totalLines += lineCount;
          stats.filesByExtension[extension].totalLines += lineCount;
        } catch (error) {
          // 无法读取文件内容，跳过行数统计
        }
      }

      // 添加到最近修改文件列表
      stats.recentFiles.push({
        path: relativePath,
        lastModified: fileStat.mtime,
        lines: lineCount,
      });
    } catch (error) {
      console.warn(`Cannot analyze file: ${filePath}`);
    }
  }

  await analyzeDirectory(rootPath);

  // 按最后修改时间排序，取最近10个
  stats.recentFiles.sort(
    (a, b) => b.lastModified.getTime() - a.lastModified.getTime(),
  );
  stats.recentFiles = stats.recentFiles.slice(0, 10);

  return stats;
}

export function formatFileSize(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

export function formatProjectStats(stats: ProjectStats): string {
  const output: Array<string> = [];

  output.push("📊 Project Statistics");
  output.push("==================");
  output.push("");

  // 总体统计
  output.push(`📁 Total Files: ${stats.totalFiles}`);
  output.push(`📝 Total Lines of Code: ${stats.totalLines.toLocaleString()}`);
  output.push(`💾 Project Size: ${formatFileSize(stats.projectSize)}`);
  output.push("");

  // 按文件类型统计
  output.push("📋 Files by Extension:");
  const sortedExtensions = Object.values(stats.filesByExtension)
    .sort((a, b) => b.count - a.count)
    .slice(0, 10); // 显示前10种文件类型

  for (const ext of sortedExtensions) {
    const percentage = ((ext.count / stats.totalFiles) * 100).toFixed(1);
    output.push(
      `  ${ext.extension.padEnd(12)} ${ext.count
        .toString()
        .padStart(4)} files (${percentage}%) - ${ext.totalLines.toLocaleString()} lines`,
    );
  }
  output.push("");

  // 最近修改的文件
  output.push("🕒 Recently Modified Files:");
  for (const file of stats.recentFiles.slice(0, 5)) {
    const timeAgo = getTimeAgo(file.lastModified);
    const lines = file.lines > 0 ? ` (${file.lines} lines)` : "";
    output.push(`  ${file.path}${lines} - ${timeAgo}`);
  }

  return output.join("\n");
}

function getTimeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 60) {
    return `${diffMins} minutes ago`;
  } else if (diffHours < 24) {
    return `${diffHours} hours ago`;
  } else if (diffDays === 1) {
    return "yesterday";
  } else {
    return `${diffDays} days ago`;
  }
}

export async function showProjectStats(jsonOutput: boolean = false): Promise<void> {
  try {
    console.log("🔍 Analyzing project...\n");

    const stats = await analyzeProject();

    if (jsonOutput) {
      console.log(JSON.stringify(stats, null, 2));
    } else {
      console.log(formatProjectStats(stats));
    }
  } catch (error) {
    console.error("❌ Error analyzing project:", error);
    process.exit(1);
  }
}