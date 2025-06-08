import fs from "fs";
import path from "path";

export interface FileSizeStats {
  extension: string;
  count: number;
  totalSize: number;
  avgSize: number;
}

export function formatBytes(bytes: number): string {
  if (bytes === 0) {
    return "0 B";
  }
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

export function getFileSizeStats(dirPath: string): Array<FileSizeStats> {
  const stats = new Map<string, { count: number; totalSize: number }>();

  function walkDirectory(currentPath: string): void {
    try {
      const entries = fs.readdirSync(currentPath, { withFileTypes: true });

      for (const entry of entries) {
        const fullPath = path.join(currentPath, entry.name);

        if (entry.isDirectory() && !entry.name.startsWith(".")) {
          walkDirectory(fullPath);
        } else if (entry.isFile()) {
          const ext = path.extname(entry.name) || "no-ext";
          const size = fs.statSync(fullPath).size;

          const current = stats.get(ext) || { count: 0, totalSize: 0 };
          stats.set(ext, {
            count: current.count + 1,
            totalSize: current.totalSize + size,
          });
        }
      }
    } catch (error) {
      // Skip directories we can't read
    }
  }

  walkDirectory(dirPath);

  return Array.from(stats.entries())
    .map(([ext, data]) => ({
      extension: ext,
      count: data.count,
      totalSize: data.totalSize,
      avgSize: Math.round(data.totalSize / data.count),
    }))
    .sort((a, b) => b.totalSize - a.totalSize);
}
