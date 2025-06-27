// Maximum output cap: either MAX_OUTPUT_LINES lines or MAX_OUTPUT_BYTES bytes,
// whichever limit is reached first.
import { DEFAULT_SHELL_MAX_BYTES, DEFAULT_SHELL_MAX_LINES } from "../../config";

/**
 * Creates a collector that accumulates data Buffers from a stream up to
 * specified byte and line limits. After either limit is exceeded, further
 * data is ignored.
 */
export function createTruncatingCollector(
  stream: NodeJS.ReadableStream,
  byteLimit: number = DEFAULT_SHELL_MAX_BYTES,
  lineLimit: number = DEFAULT_SHELL_MAX_LINES,
): {
  getString: () => string;
  hit: boolean;
} {
  const chunks: Array<Buffer> = [];
  let totalBytes = 0;
  let totalLines = 0;
  let hitLimit = false;

  stream?.on("data", (data: Buffer) => {
    if (hitLimit) {
      return;
    }
    const dataLength = data.length;
    let newlineCount = 0;
    for (let i = 0; i < dataLength; i++) {
      if (data[i] === 0x0a) {
        newlineCount++;
      }
    }
    // If entire chunk fits within byte and line limits, take it whole
    if (
      totalBytes + dataLength <= byteLimit &&
      totalLines + newlineCount <= lineLimit
    ) {
      chunks.push(data);
      totalBytes += dataLength;
      totalLines += newlineCount;
    } else {
      // Otherwise, take a partial slice up to the first limit breach
      const allowedBytes = byteLimit - totalBytes;
      const allowedLines = lineLimit - totalLines;
      let bytesTaken = 0;
      let linesSeen = 0;
      for (let i = 0; i < dataLength; i++) {
        // Stop if byte or line limit is reached
        if (bytesTaken === allowedBytes || linesSeen === allowedLines) {
          break;
        }
        const byte = data[i];
        if (byte === 0x0a) {
          linesSeen++;
        }
        bytesTaken++;
      }
      if (bytesTaken > 0) {
        chunks.push(data.slice(0, bytesTaken));
        totalBytes += bytesTaken;
        totalLines += linesSeen;
      }
      hitLimit = true;
    }
  });

  return {
    getString() {
      return Buffer.concat(chunks).toString("utf8");
    },
    /** True if either byte or line limit was exceeded */
    get hit(): boolean {
      return hitLimit;
    },
  };
}
