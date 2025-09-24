/**
 * Chrome extension background service worker
 * Central coordinator for the Codex agent
 */

import { CodexAgent } from '../core/CodexAgent';
import { MessageRouter, MessageType } from '../core/MessageRouter';
import { Submission, Event } from '../protocol/types';
import { validateSubmission } from '../protocol/schemas';

// Global agent instance
let agent: CodexAgent | null = null;
let router: MessageRouter | null = null;

/**
 * Initialize the service worker
 */
async function initialize(): Promise<void> {
  console.log('Initializing Codex background service worker');
  
  // Create agent instance
  agent = new CodexAgent();
  
  // Create message router
  router = new MessageRouter('background');
  
  // Setup message handlers
  setupMessageHandlers();
  
  // Setup Chrome event listeners
  setupChromeListeners();
  
  // Setup periodic tasks
  setupPeriodicTasks();
  
  console.log('Service worker initialized');
}

/**
 * Setup message handlers
 */
function setupMessageHandlers(): void {
  if (!router || !agent) return;
  
  // Handle submissions from UI
  router.on(MessageType.SUBMISSION, async (message) => {
    const submission = message.payload as Submission;
    
    if (!validateSubmission(submission)) {
      console.error('Invalid submission:', submission);
      return;
    }
    
    try {
      const id = await agent!.submitOperation(submission.op);
      return { submissionId: id };
    } catch (error) {
      console.error('Failed to submit operation:', error);
      throw error;
    }
  });
  
  // Handle state queries
  router.on(MessageType.GET_STATE, async () => {
    if (!agent) return null;
    
    const session = agent.getSession();
    return {
      sessionId: session.conversationId,
      messageCount: session.getMessageCount(),
      turnContext: session.getTurnContext(),
      metadata: session.getMetadata(),
    };
  });
  
  // Handle ping/pong for connection testing
  router.on(MessageType.PING, async () => {
    return { type: MessageType.PONG, timestamp: Date.now() };
  });
  
  // Handle storage operations
  router.on(MessageType.STORAGE_GET, async (message) => {
    const { key } = message.payload;
    const result = await chrome.storage.local.get(key);
    return result[key];
  });
  
  router.on(MessageType.STORAGE_SET, async (message) => {
    const { key, value } = message.payload;
    await chrome.storage.local.set({ [key]: value });
    return { success: true };
  });
  
  // Handle tab commands
  router.on(MessageType.TAB_COMMAND, async (message) => {
    const { command, args } = message.payload;
    const tabId = message.tabId;
    
    if (!tabId) {
      throw new Error('Tab ID required for tab command');
    }
    
    return executeTabCommand(tabId, command, args);
  });
}

/**
 * Setup Chrome API event listeners
 */
function setupChromeListeners(): void {
  // Handle extension installation
  chrome.runtime.onInstalled.addListener((details) => {
    console.log('Extension installed:', details.reason);
    
    if (details.reason === 'install') {
      // Open welcome page on first install
      chrome.tabs.create({
        url: chrome.runtime.getURL('welcome.html'),
      });
    }
    
    // Setup context menus
    setupContextMenus();
  });
  
  // Handle side panel opening
  if (chrome.sidePanel) {
    chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true });
  }
  
  // Handle tab updates
  chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
    if (changeInfo.status === 'complete') {
      // Inject content script if needed
      injectContentScriptIfNeeded(tabId, tab);
    }
  });
  
  // Handle commands (keyboard shortcuts)
  chrome.commands.onCommand.addListener((command) => {
    handleCommand(command);
  });
  
  // Handle context menu clicks
  chrome.contextMenus.onClicked.addListener((info, tab) => {
    handleContextMenuClick(info, tab);
  });
}

/**
 * Setup context menus
 */
function setupContextMenus(): void {
  chrome.contextMenus.create({
    id: 'codex-explain',
    title: 'Explain with Codex',
    contexts: ['selection'],
  });
  
  chrome.contextMenus.create({
    id: 'codex-improve',
    title: 'Improve with Codex',
    contexts: ['selection'],
  });
  
  chrome.contextMenus.create({
    id: 'codex-extract',
    title: 'Extract data with Codex',
    contexts: ['page', 'frame'],
  });
}

/**
 * Handle keyboard commands
 */
function handleCommand(command: string): void {
  switch (command) {
    case 'toggle-sidepanel':
      // Toggle side panel
      chrome.sidePanel.open({ windowId: chrome.windows.WINDOW_ID_CURRENT });
      break;
      
    case 'quick-action':
      // Trigger quick action on current tab
      chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
        if (tabs[0]?.id) {
          executeQuickAction(tabs[0].id);
        }
      });
      break;
  }
}

/**
 * Handle context menu clicks
 */
async function handleContextMenuClick(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab
): Promise<void> {
  if (!tab?.id || !agent) return;
  
  const submission: Partial<Submission> = {
    id: `ctx_${Date.now()}`,
    op: {
      type: 'UserInput',
      items: [],
    },
  };
  
  switch (info.menuItemId) {
    case 'codex-explain':
      if (info.selectionText) {
        submission.op = {
          type: 'UserInput',
          items: [
            {
              type: 'text',
              text: `Explain this: ${info.selectionText}`,
            },
          ],
        };
      }
      break;
      
    case 'codex-improve':
      if (info.selectionText) {
        submission.op = {
          type: 'UserInput',
          items: [
            {
              type: 'text',
              text: `Improve this text: ${info.selectionText}`,
            },
          ],
        };
      }
      break;
      
    case 'codex-extract':
      submission.op = {
        type: 'UserInput',
        items: [
          {
            type: 'text',
            text: `Extract structured data from this page`,
          },
          {
            type: 'context',
            path: info.pageUrl,
          },
        ],
      };
      break;
  }
  
  // Submit to agent
  if (submission.op) {
    await agent.submitOperation(submission.op);
    
    // Open side panel to show results
    chrome.sidePanel.open({ tabId: tab.id });
  }
}

/**
 * Inject content script if needed
 */
async function injectContentScriptIfNeeded(
  tabId: number,
  tab: chrome.tabs.Tab
): Promise<void> {
  // Skip chrome:// and other protected URLs
  if (!tab.url || tab.url.startsWith('chrome://') || tab.url.startsWith('chrome-extension://')) {
    return;
  }
  
  try {
    // Check if content script is already injected
    const response = await chrome.tabs.sendMessage(tabId, { type: 'PING' });
    if (response) {
      return; // Already injected
    }
  } catch {
    // Not injected, proceed with injection
  }
  
  // Inject content script
  try {
    await chrome.scripting.executeScript({
      target: { tabId },
      files: ['content-script.js'],
    });
  } catch (error) {
    console.error('Failed to inject content script:', error);
  }
}

/**
 * Execute tab command
 */
async function executeTabCommand(
  tabId: number,
  command: string,
  args?: any
): Promise<any> {
  switch (command) {
    case 'evaluate':
      return chrome.scripting.executeScript({
        target: { tabId },
        func: (code: string) => eval(code),
        args: [args.code],
      });
      
    case 'screenshot':
      return chrome.tabs.captureVisibleTab({ format: 'png' });
      
    case 'get-html':
      return chrome.scripting.executeScript({
        target: { tabId },
        func: () => document.documentElement.outerHTML,
      });
      
    case 'get-text':
      return chrome.scripting.executeScript({
        target: { tabId },
        func: () => document.body.innerText,
      });
      
    case 'navigate':
      return chrome.tabs.update(tabId, { url: args.url });
      
    case 'reload':
      return chrome.tabs.reload(tabId);
      
    case 'close':
      return chrome.tabs.remove(tabId);
      
    default:
      throw new Error(`Unknown tab command: ${command}`);
  }
}

/**
 * Execute quick action on tab
 */
async function executeQuickAction(tabId: number): Promise<void> {
  // Get current page context
  const [tab] = await chrome.tabs.get(tabId);
  
  if (!agent) return;
  
  // Submit quick analysis request
  await agent.submitOperation({
    type: 'UserInput',
    items: [
      {
        type: 'text',
        text: 'Analyze this page and provide key insights',
      },
      {
        type: 'context',
        path: tab.url,
      },
    ],
  });
  
  // Open side panel
  chrome.sidePanel.open({ tabId });
}

/**
 * Setup periodic tasks
 */
function setupPeriodicTasks(): void {
  // Process event queue periodically
  setInterval(async () => {
    if (!agent || !router) return;
    
    // Get next event from agent
    const event = await agent.getNextEvent();
    if (event) {
      // Broadcast event to all connected clients
      await router.broadcast(MessageType.EVENT, event);
    }
  }, 100); // Check every 100ms
  
  // Cleanup old data periodically
  setInterval(async () => {
    const storage = await chrome.storage.local.get(null);
    const now = Date.now();
    const keysToRemove: string[] = [];
    
    // Remove old temporary data (older than 24 hours)
    for (const key in storage) {
      if (key.startsWith('temp_')) {
        const data = storage[key];
        if (data.timestamp && now - data.timestamp > 24 * 60 * 60 * 1000) {
          keysToRemove.push(key);
        }
      }
    }
    
    if (keysToRemove.length > 0) {
      await chrome.storage.local.remove(keysToRemove);
    }
  }, 60 * 60 * 1000); // Every hour
}

/**
 * Handle service worker activation
 */
chrome.runtime.onStartup.addListener(() => {
  initialize();
});

// Initialize on script load
initialize();

// Export for testing
export { agent, router, initialize };
