# Slash commands

For an overview of Codex CLI slash commands, see [this documentation](https://developers.openai.com/codex/cli/slash-commands).

Local additions in this checkout:

- `/recall <query>` builds an in-memory BM25 index over saved Codex session rollouts, ranks older
  sessions against the query, and inserts the top recalled context into the composer for review
  before sending.
