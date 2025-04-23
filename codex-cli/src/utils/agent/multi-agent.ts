import { EventEmitter } from "events";
import type { ResponseItem, ResponseInputItem } from "openai/resources/responses/responses";
import { homedir } from "os";

import { AgentLoop } from "./agent-loop";
import type { AppConfig } from "../config";
import type { ApprovalPolicy } from "../../approvals";
import { ReviewDecision } from "./review";

export interface AgentSpec {
  /** Unique identifier used for routing and UI labels */
  name: string;
  /** Chat/completions model to use (e.g. "o4-mini") */
  model: string;
  /** System‑level instructions specific to this agent */
  instructions: string;
  /** Optional per‑agent approval policy (falls back to coordinator default) */
  approvalPolicy?: ApprovalPolicy;
}

export type AgentEvent = {
  /** Name of the agent that produced the message */
  from: string;
  /** The raw ResponseItem emitted by AgentLoop */
  item: ResponseItem;
};

/**
 * Simple coordinator that spawns multiple AgentLoop instances and provides a
 * message bus so callers (TUI / tests) can observe and route messages.
 *
 * The first iteration implements a broadcast strategy: every `message` item
 * produced by one agent is forwarded as user input to *all* other agents. More
 * sophisticated routing (mentions, dedicated planner→coder pipeline, …) can
 * be layered on top later without modifying the AgentLoop class itself.
 */
export class MultiAgentCoordinator {
  private agents: Map<string, AgentLoop> = new Map();
  private eventBus = new EventEmitter();
  /** Keep track of message IDs we already forwarded to avoid ping‑pong loops. */
  private readonly forwarded = new Set<string>();
  private readonly forwardedContent = new Set<string>();
  private readonly forwardedCalls = new Set<string>();
  private readonly primaryAgent: string;

  constructor(
    specs: ReadonlyArray<AgentSpec>,
    cfg: AppConfig,
    private readonly defaultApproval: ApprovalPolicy,
  ) {
    const primary = specs[0]?.name ?? "";
    for (const spec of specs) {
      if (this.agents.has(spec.name)) {
        throw new Error(`duplicate agent name: ${spec.name}`);
      }

      const loop = new AgentLoop({
        model: spec.model,
        instructions: spec.instructions,
        config: cfg,
        approvalPolicy: spec.approvalPolicy ?? this.defaultApproval,
        // allow writes inside the user's Downloads directory so coder can create files
        additionalWritableRoots: [
          `${homedir()}${process.platform === "win32" ? "\\Downloads" : "/Downloads"}`,
        ],
        // Broadcast every output item via eventBus
        onItem: (item) => this.handleItem(spec.name, item),
        onLoading: () => {}, // TODO: surface per‑agent loading state
        // Auto‑approve every command so the coder can execute without extra prompts
        getCommandConfirmation: async () => {
          return Promise.resolve({ review: ReviewDecision.ALWAYS });
        },
        onLastResponseId: () => {},
      });
      this.agents.set(spec.name, loop);
    }

    // First spec is considered the primary (e.g. planner)
    this.primaryAgent = primary;
  }

  /** Subscribe to live events emitted by any agent. */
  public onEvent(listener: (e: AgentEvent) => void): () => void {
    this.eventBus.on("event", listener);
    return () => this.eventBus.off("event", listener);
  }

  /** Send an initial prompt to one of the agents (defaults to first). */
  public async bootstrap(prompt: string, target?: string): Promise<void> {
    const tgt = (target ?? Array.from(this.agents.keys())[0]) as string;
    const loop = this.agents.get(tgt);
    if (!loop) throw new Error(`unknown agent: ${tgt}`);
    await loop.run([
      {
        type: "message",
        role: "user",
        content: [
          {
            type: "input_text",
            text: prompt,
          },
        ],
      } as unknown as ResponseInputItem,
    ]);
  }

  /** Terminates all agents and cleans up resources. */
  public terminate(): void {
    for (const loop of this.agents.values()) {
      loop.terminate();
    }
  }

  private async handleItem(agentName: string, item: ResponseItem): Promise<void> {
    this.eventBus.emit("event", { from: agentName, item } satisfies AgentEvent);

    // Forward only the FIRST assistant message (or function_call) of each planner turn
    const anyItem = item as any;
    const isPlannerReply = agentName === this.primaryAgent && anyItem.role === "assistant";
    const isAssistantMsg = item.type === "message";

    if (isPlannerReply && isAssistantMsg) {
      const hash = JSON.stringify((item as any).content ?? "");
      if (this.forwardedContent.has(hash)) return;
      this.forwardedContent.add(hash);

      const msgInput: ResponseInputItem = {
        type: "message",
        role: "user",
        content: [
          {
            type: "input_text",
            text: JSON.stringify((item as any).content ?? item),
          },
        ] as unknown as any,
      };

      const promises: Array<Promise<void>> = [];
      for (const [name, loop] of this.agents) {
        if (name === agentName) continue;
        promises.push(loop.run([msgInput]));
      }
      await Promise.allSettled(promises);
    }
  }
} 