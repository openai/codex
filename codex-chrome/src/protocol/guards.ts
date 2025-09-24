/**
 * Type guards for runtime type checking of protocol types
 */

import { Submission, Event, Op, InputItem } from './types';
import { EventMsg } from './events';

/**
 * Check if an object is a Submission
 */
export function isSubmission(obj: any): obj is Submission {
  return (
    obj &&
    typeof obj === 'object' &&
    typeof obj.id === 'string' &&
    obj.op &&
    typeof obj.op === 'object'
  );
}

/**
 * Check if an object is an Event
 */
export function isEvent(obj: any): obj is Event {
  return (
    obj &&
    typeof obj === 'object' &&
    typeof obj.id === 'string' &&
    obj.msg &&
    typeof obj.msg === 'object'
  );
}

/**
 * Check if an object is an Op
 */
export function isOp(obj: any): obj is Op {
  return obj && typeof obj === 'object' && typeof obj.type === 'string';
}

/**
 * Check if an object is an InputItem
 */
export function isInputItem(obj: any): obj is InputItem {
  return (
    obj &&
    typeof obj === 'object' &&
    typeof obj.type === 'string' &&
    ['text', 'image', 'clipboard', 'context'].includes(obj.type)
  );
}

/**
 * Check if an object is an EventMsg
 */
export function isEventMsg(obj: any): obj is EventMsg {
  return obj && typeof obj === 'object' && typeof obj.type === 'string';
}

/**
 * Type guard for specific Op types
 */
export function isUserInputOp(op: Op): op is Extract<Op, { type: 'UserInput' }> {
  return op.type === 'UserInput';
}

export function isUserTurnOp(op: Op): op is Extract<Op, { type: 'UserTurn' }> {
  return op.type === 'UserTurn';
}

export function isInterruptOp(op: Op): op is Extract<Op, { type: 'Interrupt' }> {
  return op.type === 'Interrupt';
}

/**
 * Type guard for specific EventMsg types
 */
export function isTaskStartedEvent(
  msg: EventMsg
): msg is Extract<EventMsg, { type: 'TaskStarted' }> {
  return msg.type === 'TaskStarted';
}

export function isTaskCompleteEvent(
  msg: EventMsg
): msg is Extract<EventMsg, { type: 'TaskComplete' }> {
  return msg.type === 'TaskComplete';
}

export function isAgentMessageEvent(
  msg: EventMsg
): msg is Extract<EventMsg, { type: 'AgentMessage' }> {
  return msg.type === 'AgentMessage';
}

export function isErrorEvent(msg: EventMsg): msg is Extract<EventMsg, { type: 'Error' }> {
  return msg.type === 'Error';
}