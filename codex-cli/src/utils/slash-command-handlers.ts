/**
 * Helper to execute built-in slash commands.  Returns true if the input was
 * handled as a slash command, false otherwise.
 */
export interface SlashCommandHandlers {
  setInput: (text: string) => void;
  openOverlay: () => void;
  openHelpOverlay: () => void;
  openDiffOverlay: () => void;
  onCompact: () => void;
  openModelOverlay: () => void;
  openApprovalOverlay: () => void;
  toggleFlexMode: () => void;
}

export function handleSlashCommand(
  inputValue: string,
  {
    setInput,
    openOverlay,
    openHelpOverlay,
    openDiffOverlay,
    onCompact,
    openModelOverlay,
    openApprovalOverlay,
    toggleFlexMode,
  }: SlashCommandHandlers,
): boolean {
  const cmd = inputValue.trim();
  switch (cmd) {
    case "/history":
      setInput("");
      openOverlay();
      return true;
    case "/help":
      setInput("");
      openHelpOverlay();
      return true;
    case "/diff":
      setInput("");
      openDiffOverlay();
      return true;
    case "/compact":
      setInput("");
      onCompact();
      return true;
    case "/flex-mode":
      setInput("");
      toggleFlexMode();
      return true;
    default:
      if (cmd.startsWith("/model")) {
        setInput("");
        openModelOverlay();
        return true;
      }
      if (cmd.startsWith("/approval")) {
        setInput("");
        openApprovalOverlay();
        return true;
      }
      break;
  }
  return false;
}
