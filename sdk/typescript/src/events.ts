// based on event types from codex-rs/exec/src/exec_events.rs

import type { ConversationItem } from "./items";

export type SessionCreatedEvent = {
  type: "session.created";
  session_id: string;
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

export type ItemStartedEvent = {
  type: "item.started";
  item: ConversationItem;
};

export type ItemUpdatedEvent = {
  type: "item.updated";
  item: ConversationItem;
};

export type ItemCompletedEvent = {
  type: "item.completed";
  item: ConversationItem;
};

export type ConversationErrorEvent = {
  type: "error";
  message: string;
};

export type ConversationEvent =
  | SessionCreatedEvent
  | TurnStartedEvent
  | TurnCompletedEvent
  | ItemStartedEvent
  | ItemUpdatedEvent
  | ItemCompletedEvent
  | ConversationErrorEvent;
