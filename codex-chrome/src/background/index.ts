/**
 * Background entry point for Chrome extension
 * Re-exports service worker functionality
 */

export * from './service-worker';
export { CodexAgent } from '../core/CodexAgent';
export { MessageRouter, MessageType } from '../core/MessageRouter';
export { ModelClientFactory } from '../models/ModelClientFactory';
export { ToolRegistry } from '../tools/ToolRegistry';

// For convenience, also export the main service worker initialization
export { initialize as initializeBackground } from './service-worker';