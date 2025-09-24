/**
 * Content script entry point for Chrome extension
 * Re-exports content script functionality
 */

export * from './content-script';
export { MessageRouter, MessageType } from '../core/MessageRouter';

// For convenience, export key functions
export {
  getPageContext,
  selectElements,
  executeCommand,
  executeDOMTool
} from './content-script';