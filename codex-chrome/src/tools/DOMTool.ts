/**
 * DOM Tool
 *
 * Provides DOM manipulation capabilities through content script communication.
 * Handles element querying, clicking, typing, attribute manipulation, and text extraction.
 */

import { BaseTool, BaseToolRequest, BaseToolOptions, createToolDefinition } from './BaseTool';
import { ToolDefinition } from './ToolRegistry';

/**
 * DOM tool request interface
 */
export interface DOMToolRequest extends BaseToolRequest {
  action: 'query' | 'click' | 'type' | 'getAttribute' | 'setAttribute' | 'getText' | 'getHtml' | 'submit' | 'focus' | 'scroll';
  tabId?: number;
  selector?: string;
  text?: string;
  attribute?: string;
  value?: string;
  options?: DOMActionOptions;
}

/**
 * DOM action options
 */
export interface DOMActionOptions {
  waitFor?: number;
  scrollIntoView?: boolean;
  force?: boolean;
  timeout?: number;
  delay?: number;
  clear?: boolean;
  multiple?: boolean;
  frameSelector?: string;
}

/**
 * DOM element information
 */
export interface DOMElement {
  tagName: string;
  id?: string;
  className?: string;
  textContent?: string;
  innerHTML?: string;
  outerHTML?: string;
  attributes: Record<string, string>;
  boundingBox?: BoundingBox;
  visible?: boolean;
  enabled?: boolean;
  focused?: boolean;
}

/**
 * Element bounding box
 */
export interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
  top: number;
  left: number;
  bottom: number;
  right: number;
}

/**
 * DOM tool response data
 */
export interface DOMToolResponse {
  elements?: DOMElement[];
  element?: DOMElement;
  text?: string;
  html?: string;
  attribute?: string;
  success?: boolean;
  count?: number;
}

/**
 * Content script message
 */
interface ContentScriptMessage {
  type: 'DOM_ACTION';
  action: string;
  data: any;
  requestId: string;
}

/**
 * Content script response
 */
interface ContentScriptResponse {
  success: boolean;
  data?: any;
  error?: string;
  requestId: string;
}

/**
 * DOM Tool Implementation
 *
 * Communicates with content scripts to perform DOM operations.
 */
export class DOMTool extends BaseTool {
  protected toolDefinition: ToolDefinition = createToolDefinition(
    'browser_dom',
    'Interact with DOM elements - query, click, type, get/set attributes and text',
    {
      action: {
        type: 'string',
        description: 'The DOM action to perform',
        enum: ['query', 'click', 'type', 'getAttribute', 'setAttribute', 'getText', 'getHtml', 'submit', 'focus', 'scroll'],
      },
      tabId: {
        type: 'number',
        description: 'Tab ID to perform action on (uses active tab if not specified)',
      },
      selector: {
        type: 'string',
        description: 'CSS selector to target elements',
      },
      text: {
        type: 'string',
        description: 'Text to type or search for',
      },
      attribute: {
        type: 'string',
        description: 'Attribute name for get/set operations',
      },
      value: {
        type: 'string',
        description: 'Value to set for attributes or form fields',
      },
      options: {
        type: 'object',
        description: 'Additional options for DOM actions',
        properties: {
          waitFor: { type: 'number', description: 'Time to wait before action (ms)', default: 0 },
          scrollIntoView: { type: 'boolean', description: 'Scroll element into view', default: false },
          force: { type: 'boolean', description: 'Force action even if element not visible', default: false },
          timeout: { type: 'number', description: 'Timeout for action (ms)', default: 5000 },
          delay: { type: 'number', description: 'Delay between keystrokes for typing (ms)', default: 0 },
          clear: { type: 'boolean', description: 'Clear field before typing', default: false },
          multiple: { type: 'boolean', description: 'Return multiple elements for query', default: false },
          frameSelector: { type: 'string', description: 'CSS selector for iframe to target' },
        },
      },
    },
    {
      required: ['action'],
      category: 'dom',
      version: '1.0.0',
      metadata: {
        capabilities: ['dom_manipulation', 'element_interaction', 'text_extraction'],
        permissions: ['activeTab', 'scripting'],
      },
    }
  );

  private pendingRequests: Map<string, { resolve: (value: any) => void; reject: (error: any) => void }> = new Map();

  constructor() {
    super();
    this.setupMessageListener();
  }

  /**
   * Execute DOM tool action
   */
  protected async executeImpl(request: DOMToolRequest, options?: BaseToolOptions): Promise<DOMToolResponse> {
    // Validate Chrome context
    this.validateChromeContext();

    // Validate required permissions
    await this.validatePermissions(['activeTab', 'scripting']);

    this.log('debug', `Executing DOM action: ${request.action}`, request);

    // Get target tab
    const targetTab = request.tabId ? await this.validateTabId(request.tabId) : await this.getActiveTab();

    // Ensure content script is injected
    await this.ensureContentScriptInjected(targetTab.id!);

    switch (request.action) {
      case 'query':
        return this.queryElements(targetTab.id!, request);

      case 'click':
        return this.clickElement(targetTab.id!, request);

      case 'type':
        return this.typeText(targetTab.id!, request);

      case 'getAttribute':
        return this.getAttribute(targetTab.id!, request);

      case 'setAttribute':
        return this.setAttribute(targetTab.id!, request);

      case 'getText':
        return this.getText(targetTab.id!, request);

      case 'getHtml':
        return this.getHtml(targetTab.id!, request);

      case 'submit':
        return this.submitForm(targetTab.id!, request);

      case 'focus':
        return this.focusElement(targetTab.id!, request);

      case 'scroll':
        return this.scrollToElement(targetTab.id!, request);

      default:
        throw new Error(`Unsupported DOM action: ${request.action}`);
    }
  }

  /**
   * Query DOM elements
   */
  private async queryElements(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for query action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'query', {
      selector: request.selector,
      options: request.options,
    });

    return {
      elements: result.elements || [],
      count: result.elements?.length || 0,
    };
  }

  /**
   * Click an element
   */
  private async clickElement(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for click action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'click', {
      selector: request.selector,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Type text into an element
   */
  private async typeText(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for type action');
    }

    if (!request.text) {
      throw new Error('Text is required for type action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'type', {
      selector: request.selector,
      text: request.text,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Get element attribute
   */
  private async getAttribute(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for getAttribute action');
    }

    if (!request.attribute) {
      throw new Error('Attribute name is required for getAttribute action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'getAttribute', {
      selector: request.selector,
      attribute: request.attribute,
      options: request.options,
    });

    return {
      attribute: result.value,
      element: result.element,
    };
  }

  /**
   * Set element attribute
   */
  private async setAttribute(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for setAttribute action');
    }

    if (!request.attribute) {
      throw new Error('Attribute name is required for setAttribute action');
    }

    if (request.value === undefined) {
      throw new Error('Value is required for setAttribute action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'setAttribute', {
      selector: request.selector,
      attribute: request.attribute,
      value: request.value,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Get element text content
   */
  private async getText(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for getText action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'getText', {
      selector: request.selector,
      options: request.options,
    });

    return {
      text: result.text,
      element: result.element,
    };
  }

  /**
   * Get element HTML
   */
  private async getHtml(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for getHtml action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'getHtml', {
      selector: request.selector,
      options: request.options,
    });

    return {
      html: result.html,
      element: result.element,
    };
  }

  /**
   * Submit a form
   */
  private async submitForm(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for submit action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'submit', {
      selector: request.selector,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Focus an element
   */
  private async focusElement(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for focus action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'focus', {
      selector: request.selector,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Scroll to element
   */
  private async scrollToElement(tabId: number, request: DOMToolRequest): Promise<DOMToolResponse> {
    if (!request.selector) {
      throw new Error('Selector is required for scroll action');
    }

    const result = await this.sendContentScriptMessage(tabId, 'scroll', {
      selector: request.selector,
      options: request.options,
    });

    return {
      element: result.element,
      success: result.success,
    };
  }

  /**
   * Send message to content script
   */
  private async sendContentScriptMessage(tabId: number, action: string, data: any): Promise<any> {
    const requestId = this.generateRequestId();
    const message: ContentScriptMessage = {
      type: 'DOM_ACTION',
      action,
      data,
      requestId,
    };

    return new Promise((resolve, reject) => {
      this.pendingRequests.set(requestId, { resolve, reject });

      // Set timeout for the request
      const timeout = data.options?.timeout || 5000;
      const timeoutId = setTimeout(() => {
        this.pendingRequests.delete(requestId);
        reject(new Error(`DOM action '${action}' timed out after ${timeout}ms`));
      }, timeout);

      chrome.tabs.sendMessage(tabId, message, (response) => {
        clearTimeout(timeoutId);
        this.pendingRequests.delete(requestId);

        if (chrome.runtime.lastError) {
          reject(new Error(`Content script communication failed: ${chrome.runtime.lastError.message}`));
          return;
        }

        if (!response) {
          reject(new Error('No response from content script'));
          return;
        }

        if (!response.success) {
          reject(new Error(response.error || 'DOM action failed'));
          return;
        }

        resolve(response.data);
      });
    });
  }

  /**
   * Ensure content script is injected into the tab
   */
  private async ensureContentScriptInjected(tabId: number): Promise<void> {
    try {
      // Check if content script is already available
      const response = await chrome.tabs.sendMessage(tabId, { type: 'PING' });
      if (response && response.type === 'PONG') {
        return; // Content script is already loaded
      }
    } catch (error) {
      // Content script not loaded, inject it
    }

    try {
      await chrome.scripting.executeScript({
        target: { tabId },
        files: ['/content/content-script.js'],
      });

      // Wait a moment for the script to initialize
      await new Promise(resolve => setTimeout(resolve, 100));

      this.log('info', `Content script injected into tab ${tabId}`);
    } catch (error) {
      throw new Error(`Failed to inject content script: ${error}`);
    }
  }

  /**
   * Setup message listener for content script responses
   */
  private setupMessageListener(): void {
    if (chrome.runtime && chrome.runtime.onMessage) {
      chrome.runtime.onMessage.addListener((message: ContentScriptResponse, sender, sendResponse) => {
        if (message.requestId && this.pendingRequests.has(message.requestId)) {
          const pending = this.pendingRequests.get(message.requestId)!;
          this.pendingRequests.delete(message.requestId);

          if (message.success) {
            pending.resolve(message.data);
          } else {
            pending.reject(new Error(message.error || 'Content script action failed'));
          }
        }
      });
    }
  }

  /**
   * Generate unique request ID
   */
  private generateRequestId(): string {
    return `dom_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Wait for element to appear
   */
  async waitForElement(tabId: number, selector: string, timeout: number = 5000): Promise<DOMElement | null> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const result = await this.sendContentScriptMessage(tabId, 'query', {
          selector,
          options: { multiple: false },
        });

        if (result.elements && result.elements.length > 0) {
          return result.elements[0];
        }
      } catch (error) {
        // Continue waiting
      }

      await new Promise(resolve => setTimeout(resolve, 100));
    }

    return null;
  }

  /**
   * Wait for element to be visible
   */
  async waitForVisible(tabId: number, selector: string, timeout: number = 5000): Promise<DOMElement | null> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const result = await this.sendContentScriptMessage(tabId, 'query', {
          selector,
          options: { multiple: false },
        });

        if (result.elements && result.elements.length > 0 && result.elements[0].visible) {
          return result.elements[0];
        }
      } catch (error) {
        // Continue waiting
      }

      await new Promise(resolve => setTimeout(resolve, 100));
    }

    return null;
  }

  /**
   * Execute multiple DOM actions in sequence
   */
  async executeSequence(tabId: number, actions: Omit<DOMToolRequest, 'tabId'>[]): Promise<DOMToolResponse[]> {
    const results: DOMToolResponse[] = [];

    for (const action of actions) {
      try {
        const result = await this.executeImpl({ ...action, tabId });
        results.push(result);
      } catch (error) {
        // Add error result and continue
        results.push({
          success: false,
          // error: error instanceof Error ? error.message : String(error),
        });
      }
    }

    return results;
  }

  /**
   * Extract all text from page
   */
  async extractPageText(tabId: number): Promise<string> {
    const result = await this.sendContentScriptMessage(tabId, 'extractText', {});
    return result.text || '';
  }

  /**
   * Extract all links from page
   */
  async extractLinks(tabId: number, selector?: string): Promise<Array<{ text: string; href: string; title?: string }>> {
    const result = await this.sendContentScriptMessage(tabId, 'extractLinks', {
      selector: selector || 'a[href]',
    });
    return result.links || [];
  }

  /**
   * Fill form with data
   */
  async fillForm(tabId: number, formData: Record<string, string>, formSelector?: string): Promise<DOMToolResponse> {
    const result = await this.sendContentScriptMessage(tabId, 'fillForm', {
      formData,
      formSelector,
    });

    return {
      success: result.success,
      count: result.fieldsSet || 0,
    };
  }
}