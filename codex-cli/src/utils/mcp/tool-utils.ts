/**
 * Parses a prefixed tool name into server name and original tool name
 * @param prefixedName The prefixed tool name in format "serverName:toolName"
 * @returns Object with serverName and toolName, or null if invalid format
 */
export function parsePrefixedToolName(
  prefixedName: string,
): { serverName: string; toolName: string } | null {
  const separatorIndex = prefixedName.indexOf("_");
  if (
    separatorIndex === -1 ||
    separatorIndex === 0 ||
    separatorIndex === prefixedName.length - 1
  ) {
    return null; // Not a valid prefixed name
  }
  return {
    serverName: prefixedName.substring(0, separatorIndex),
    toolName: prefixedName.substring(separatorIndex + 1),
  };
}
