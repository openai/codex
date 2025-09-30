// based on event types from codex-rs/exec/src/exec_events.rs

import type { ThreadItem } from "./items";

export type ThreadStartedEvent = {
  type: "thread.started";
  thread_id: string;
};

export type TurnStartedEvent = {
  type: "turn.started";
};

export type Usage = {
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
};

export type TurnCompletedEvent = {
  type: "turn.completed";
  usage: Usage;
};

export type TurnFailedEvent = {
  type: "turn.failed";
  error: ThreadError;
};

export type ItemStartedEvent = {
  type: "item.started";
  item: ThreadItem;
};

export type ItemUpdatedEvent = {
  type: "item.updated";
  item: ThreadItem;
};

export type ItemCompletedEvent = {
  type: "item.completed";
  item: ThreadItem;
};

export type ThreadError = {
  message: string;
};

export type ThreadErrorEvent = {
  type: "error";
  message: string;
};

export type ThreadEvent =
  | ThreadStartedEvent
  | TurnStartedEvent
  | TurnCompletedEvent
  | TurnFailedEvent
  | ItemStartedEvent
  | ItemUpdatedEvent
  | ItemCompletedEvent
  | ThreadErrorEvent;
