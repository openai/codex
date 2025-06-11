export function setGitHubActionsUser(): void {
  const commands = [
    ["git", "config", "--global", "user.name", "github-actions[bot]"],
    [
      "git",
      "config",
      "--global",
      "user.email",
      "41898282+github-actions[bot]@users.noreply.github.com",
    ],
  ];

  for (const command of commands) {
    Bun.spawnSync(command);
  }
}
