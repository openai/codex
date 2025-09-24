/**
 * Content script injected into web pages
 * Provides DOM access and page interaction capabilities
 */

import { MessageRouter, MessageType } from '../core/MessageRouter';

// Router instance
let router: MessageRouter | null = null;

/**
 * Page context information
 */
interface PageContext {
  url: string;
  title: string;
  domain: string;
  protocol: string;
  pathname: string;
  search: string;
  hash: string;
  viewport: {
    width: number;
    height: number;
    scrollX: number;
    scrollY: number;
  };
  metadata: Record<string, string>;
}

/**
 * Initialize content script
 */
function initialize(): void {
  console.log('Codex content script initialized');
  
  // Create message router
  router = new MessageRouter('content');
  
  // Setup message handlers
  setupMessageHandlers();
  
  // Setup DOM observers
  setupDOMObservers();
  
  // Setup interaction handlers
  setupInteractionHandlers();
  
  // Announce presence to background
  announcePresence();
}

/**
 * Setup message handlers
 */
function setupMessageHandlers(): void {
  if (!router) return;
  
  // Handle ping (for checking if content script is loaded)
  router.on(MessageType.PING, () => {
    return { type: MessageType.PONG, timestamp: Date.now() };
  });
  
  // Handle tab commands
  router.on(MessageType.TAB_COMMAND, async (message) => {
    const { command, args } = message.payload;
    return executeCommand(command, args);
  });
  
  // Handle DOM queries
  router.on(MessageType.TAB_COMMAND, async (message) => {
    if (message.payload.command === 'query') {
      return queryDOM(message.payload.args);
    }
  });
}

/**
 * Execute command on the page
 */
function executeCommand(command: string, args?: any): any {
  switch (command) {
    case 'get-context':
      return getPageContext();
      
    case 'select':
      return selectElements(args.selector);
      
    case 'click':
      return clickElement(args.selector);
      
    case 'type':
      return typeInElement(args.selector, args.text);
      
    case 'extract':
      return extractData(args.selector, args.attributes);
      
    case 'screenshot-element':
      return screenshotElement(args.selector);
      
    case 'highlight':
      return highlightElements(args.selector, args.style);
      
    case 'remove-highlight':
      return removeHighlights();
      
    case 'scroll-to':
      return scrollToElement(args.selector);
      
    case 'get-form-data':
      return getFormData(args.selector);
      
    case 'fill-form':
      return fillForm(args.selector, args.data);
      
    case 'observe':
      return observeElement(args.selector, args.options);
      
    default:
      throw new Error(`Unknown command: ${command}`);
  }
}

/**
 * Get page context information
 */
function getPageContext(): PageContext {
  const location = window.location;
  
  // Extract metadata
  const metadata: Record<string, string> = {};
  
  // Get meta tags
  document.querySelectorAll('meta').forEach(meta => {
    const name = meta.getAttribute('name') || meta.getAttribute('property');
    const content = meta.getAttribute('content');
    if (name && content) {
      metadata[name] = content;
    }
  });
  
  return {
    url: location.href,
    title: document.title,
    domain: location.hostname,
    protocol: location.protocol,
    pathname: location.pathname,
    search: location.search,
    hash: location.hash,
    viewport: {
      width: window.innerWidth,
      height: window.innerHeight,
      scrollX: window.scrollX,
      scrollY: window.scrollY,
    },
    metadata,
  };
}

/**
 * Select elements on the page
 */
function selectElements(selector: string): any[] {
  const elements = document.querySelectorAll(selector);
  return Array.from(elements).map(el => ({
    tagName: el.tagName.toLowerCase(),
    id: el.id,
    className: el.className,
    text: (el as HTMLElement).innerText?.substring(0, 100),
    attributes: getElementAttributes(el),
  }));
}

/**
 * Click an element
 */
function clickElement(selector: string): boolean {
  const element = document.querySelector(selector) as HTMLElement;
  if (element) {
    element.click();
    return true;
  }
  return false;
}

/**
 * Type text into an element
 */
function typeInElement(selector: string, text: string): boolean {
  const element = document.querySelector(selector) as HTMLInputElement;
  if (element) {
    element.focus();
    element.value = text;
    
    // Trigger input events
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
    
    return true;
  }
  return false;
}

/**
 * Extract data from elements
 */
function extractData(
  selector: string,
  attributes?: string[]
): any[] {
  const elements = document.querySelectorAll(selector);
  return Array.from(elements).map(el => {
    const data: any = {
      text: (el as HTMLElement).innerText,
    };
    
    if (attributes) {
      attributes.forEach(attr => {
        data[attr] = el.getAttribute(attr);
      });
    } else {
      // Get all attributes
      data.attributes = getElementAttributes(el);
    }
    
    return data;
  });
}

/**
 * Get element attributes
 */
function getElementAttributes(element: Element): Record<string, string> {
  const attrs: Record<string, string> = {};
  for (const attr of element.attributes) {
    attrs[attr.name] = attr.value;
  }
  return attrs;
}

/**
 * Take screenshot of element
 */
async function screenshotElement(selector: string): Promise<string | null> {
  const element = document.querySelector(selector) as HTMLElement;
  if (!element) return null;
  
  // Get element bounds
  const rect = element.getBoundingClientRect();
  
  // Return bounds for background script to capture
  return JSON.stringify({
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
  });
}

/**
 * Highlight elements on the page
 */
function highlightElements(
  selector: string,
  style?: Partial<CSSStyleDeclaration>
): number {
  const elements = document.querySelectorAll(selector);
  let count = 0;
  
  elements.forEach(el => {
    const htmlEl = el as HTMLElement;
    
    // Store original style
    htmlEl.setAttribute('data-codex-original-style', htmlEl.getAttribute('style') || '');
    
    // Apply highlight
    htmlEl.style.outline = style?.outline || '2px solid red';
    htmlEl.style.backgroundColor = style?.backgroundColor || 'rgba(255, 255, 0, 0.3)';
    htmlEl.classList.add('codex-highlighted');
    
    count++;
  });
  
  return count;
}

/**
 * Remove all highlights
 */
function removeHighlights(): number {
  const elements = document.querySelectorAll('.codex-highlighted');
  let count = 0;
  
  elements.forEach(el => {
    const htmlEl = el as HTMLElement;
    
    // Restore original style
    const originalStyle = htmlEl.getAttribute('data-codex-original-style');
    if (originalStyle) {
      htmlEl.setAttribute('style', originalStyle);
    } else {
      htmlEl.removeAttribute('style');
    }
    
    htmlEl.removeAttribute('data-codex-original-style');
    htmlEl.classList.remove('codex-highlighted');
    
    count++;
  });
  
  return count;
}

/**
 * Scroll to element
 */
function scrollToElement(selector: string): boolean {
  const element = document.querySelector(selector);
  if (element) {
    element.scrollIntoView({
      behavior: 'smooth',
      block: 'center',
    });
    return true;
  }
  return false;
}

/**
 * Get form data
 */
function getFormData(selector: string): Record<string, any> | null {
  const form = document.querySelector(selector) as HTMLFormElement;
  if (!form) return null;
  
  const formData = new FormData(form);
  const data: Record<string, any> = {};
  
  formData.forEach((value, key) => {
    if (data[key]) {
      // Handle multiple values
      if (!Array.isArray(data[key])) {
        data[key] = [data[key]];
      }
      data[key].push(value);
    } else {
      data[key] = value;
    }
  });
  
  return data;
}

/**
 * Fill form with data
 */
function fillForm(
  selector: string,
  data: Record<string, any>
): boolean {
  const form = document.querySelector(selector) as HTMLFormElement;
  if (!form) return false;
  
  for (const [name, value] of Object.entries(data)) {
    const input = form.elements.namedItem(name) as HTMLInputElement;
    if (input) {
      if (input.type === 'checkbox' || input.type === 'radio') {
        input.checked = Boolean(value);
      } else {
        input.value = String(value);
      }
      
      // Trigger events
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }
  }
  
  return true;
}

/**
 * Query DOM with complex selectors
 */
function queryDOM(args: {
  selector?: string;
  xpath?: string;
  text?: string;
  regex?: string;
}): any[] {
  let elements: Element[] = [];
  
  if (args.selector) {
    elements = Array.from(document.querySelectorAll(args.selector));
  } else if (args.xpath) {
    const result = document.evaluate(
      args.xpath,
      document,
      null,
      XPathResult.ORDERED_NODE_SNAPSHOT_TYPE,
      null
    );
    
    for (let i = 0; i < result.snapshotLength; i++) {
      const node = result.snapshotItem(i);
      if (node && node.nodeType === Node.ELEMENT_NODE) {
        elements.push(node as Element);
      }
    }
  } else if (args.text) {
    // Find elements containing text
    const allElements = document.querySelectorAll('*');
    elements = Array.from(allElements).filter(el => {
      return (el as HTMLElement).innerText?.includes(args.text!);
    });
  }
  
  // Apply regex filter if provided
  if (args.regex && elements.length > 0) {
    const regex = new RegExp(args.regex);
    elements = elements.filter(el => {
      return regex.test((el as HTMLElement).innerText || '');
    });
  }
  
  return elements.map(el => ({
    tagName: el.tagName.toLowerCase(),
    id: el.id,
    className: el.className,
    text: (el as HTMLElement).innerText?.substring(0, 100),
  }));
}

/**
 * Setup DOM mutation observers
 */
function setupDOMObservers(): void {
  const observers: Map<string, MutationObserver> = new Map();
  
  // Store observers for cleanup
  (window as any).__codexObservers = observers;
}

/**
 * Observe element changes
 */
function observeElement(
  selector: string,
  options?: MutationObserverInit
): boolean {
  const element = document.querySelector(selector);
  if (!element) return false;
  
  const observers = (window as any).__codexObservers as Map<string, MutationObserver>;
  
  // Stop existing observer for this selector
  if (observers.has(selector)) {
    observers.get(selector)!.disconnect();
  }
  
  // Create new observer
  const observer = new MutationObserver((mutations) => {
    // Send mutations to background
    if (router) {
      router.send(MessageType.TAB_RESULT, {
        type: 'mutation',
        selector,
        mutations: mutations.map(m => ({
          type: m.type,
          target: (m.target as Element).tagName?.toLowerCase(),
          addedNodes: m.addedNodes.length,
          removedNodes: m.removedNodes.length,
        })),
      });
    }
  });
  
  observer.observe(element, options || {
    childList: true,
    attributes: true,
    subtree: true,
  });
  
  observers.set(selector, observer);
  return true;
}

/**
 * Setup interaction handlers
 */
function setupInteractionHandlers(): void {
  // Track user interactions
  let lastInteraction: any = null;
  
  // Click tracking
  document.addEventListener('click', (e) => {
    const target = e.target as HTMLElement;
    lastInteraction = {
      type: 'click',
      target: getElementSelector(target),
      timestamp: Date.now(),
    };
  }, true);
  
  // Input tracking
  document.addEventListener('input', (e) => {
    const target = e.target as HTMLInputElement;
    lastInteraction = {
      type: 'input',
      target: getElementSelector(target),
      value: target.value,
      timestamp: Date.now(),
    };
  }, true);
  
  // Store for access
  (window as any).__codexLastInteraction = () => lastInteraction;
}

/**
 * Get unique selector for element
 */
function getElementSelector(element: Element): string {
  if (element.id) {
    return `#${element.id}`;
  }
  
  const path: string[] = [];
  let current: Element | null = element;
  
  while (current && current !== document.body) {
    let selector = current.tagName.toLowerCase();
    
    if (current.className) {
      selector += `.${Array.from(current.classList).join('.')}`;
    }
    
    // Add nth-child if needed
    const parent = current.parentElement;
    if (parent) {
      const siblings = Array.from(parent.children);
      const index = siblings.indexOf(current);
      if (siblings.filter(s => s.tagName === current!.tagName).length > 1) {
        selector += `:nth-child(${index + 1})`;
      }
    }
    
    path.unshift(selector);
    current = current.parentElement;
  }
  
  return path.join(' > ');
}

/**
 * Announce presence to background script
 */
function announcePresence(): void {
  if (!router) return;
  
  // Send initial context
  router.send(MessageType.TAB_RESULT, {
    type: 'content-script-ready',
    context: getPageContext(),
  });
}

/**
 * Cleanup on unload
 */
window.addEventListener('unload', () => {
  // Disconnect observers
  const observers = (window as any).__codexObservers as Map<string, MutationObserver>;
  if (observers) {
    observers.forEach(observer => observer.disconnect());
    observers.clear();
  }
  
  // Clean up router
  if (router) {
    router.cleanup();
  }
});

// Initialize content script
initialize();

// Export for testing
export { getPageContext, selectElements, executeCommand };
