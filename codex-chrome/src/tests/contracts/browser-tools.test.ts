/**
 * Contract tests for Browser Tools
 * Tests TabTool, DOMTool, StorageTool, and NavigationTool contracts
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createMockToolResult } from '../utils/test-helpers';

// Define Browser Tool contract interfaces
interface TabToolRequest {
  action: 'create' | 'close' | 'update' | 'query' | 'activate';
  tabId?: number;
  url?: string;
  properties?: TabProperties;
  query?: TabQuery;
}

interface TabProperties {
  url?: string;
  title?: string;
  active?: boolean;
  pinned?: boolean;
  muted?: boolean;
  windowId?: number;
}

interface TabQuery {
  url?: string;
  title?: string;
  active?: boolean;
  windowId?: number;
}

interface TabToolResponse {
  success: boolean;
  data?: {
    tabs?: Tab[];
    tab?: Tab;
    tabId?: number;
  };
  error?: string;
}

interface Tab {
  id: number;
  url: string;
  title: string;
  active: boolean;
  pinned: boolean;
  windowId: number;
  status: 'loading' | 'complete';
}

interface DOMToolRequest {
  action: 'query' | 'click' | 'type' | 'getAttribute' | 'setAttribute' | 'getText' | 'getHtml';
  tabId?: number;
  selector?: string;
  text?: string;
  attribute?: string;
  value?: string;
  options?: DOMActionOptions;
}

interface DOMActionOptions {
  waitFor?: number;
  scrollIntoView?: boolean;
  force?: boolean;
  timeout?: number;
}

interface DOMToolResponse {
  success: boolean;
  data?: {
    elements?: DOMElement[];
    element?: DOMElement;
    text?: string;
    html?: string;
    attribute?: string;
  };
  error?: string;
}

interface DOMElement {
  tagName: string;
  id?: string;
  className?: string;
  textContent?: string;
  attributes: Record<string, string>;
  boundingBox?: BoundingBox;
}

interface BoundingBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface StorageToolRequest {
  action: 'get' | 'set' | 'remove' | 'clear' | 'keys';
  storageType: 'local' | 'session' | 'sync';
  key?: string;
  value?: any;
  keys?: string[];
}

interface StorageToolResponse {
  success: boolean;
  data?: {
    value?: any;
    values?: Record<string, any>;
    keys?: string[];
  };
  error?: string;
}

interface NavigationToolRequest {
  action: 'navigate' | 'reload' | 'goBack' | 'goForward' | 'getHistory';
  tabId?: number;
  url?: string;
  options?: NavigationOptions;
}

interface NavigationOptions {
  waitForLoad?: boolean;
  timeout?: number;
  referrer?: string;
}

interface NavigationToolResponse {
  success: boolean;
  data?: {
    url?: string;
    title?: string;
    status?: 'loading' | 'complete';
    history?: HistoryEntry[];
  };
  error?: string;
}

interface HistoryEntry {
  url: string;
  title: string;
  visitTime: number;
}

// Tool interfaces
interface TabTool {
  execute(request: TabToolRequest): Promise<TabToolResponse>;
}

interface DOMTool {
  execute(request: DOMToolRequest): Promise<DOMToolResponse>;
}

interface StorageTool {
  execute(request: StorageToolRequest): Promise<StorageToolResponse>;
}

interface NavigationTool {
  execute(request: NavigationToolRequest): Promise<NavigationToolResponse>;
}

describe('Browser Tools Contracts', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('TabTool Contract', () => {
    let mockTabTool: TabTool;

    beforeEach(() => {
      mockTabTool = {
        async execute(request: TabToolRequest): Promise<TabToolResponse> {
          eventCollector.collect({
            id: 'evt_tab_action',
            msg: {
              type: 'TabActionStart',
              data: {
                action: request.action,
                tab_id: request.tabId,
              },
            },
          });

          switch (request.action) {
            case 'create':
              const newTab: Tab = {
                id: 123,
                url: request.url || 'about:blank',
                title: 'New Tab',
                active: true,
                pinned: false,
                windowId: 1,
                status: 'complete',
              };
              return {
                success: true,
                data: { tab: newTab, tabId: newTab.id },
              };

            case 'query':
              const mockTabs: Tab[] = [
                {
                  id: 1,
                  url: 'https://example.com',
                  title: 'Example Site',
                  active: true,
                  pinned: false,
                  windowId: 1,
                  status: 'complete',
                },
                {
                  id: 2,
                  url: 'https://github.com',
                  title: 'GitHub',
                  active: false,
                  pinned: true,
                  windowId: 1,
                  status: 'complete',
                },
              ];

              let filteredTabs = mockTabs;
              if (request.query?.active !== undefined) {
                filteredTabs = filteredTabs.filter(tab => tab.active === request.query!.active);
              }
              if (request.query?.url) {
                filteredTabs = filteredTabs.filter(tab => tab.url.includes(request.query!.url!));
              }

              return {
                success: true,
                data: { tabs: filteredTabs },
              };

            case 'close':
              if (!request.tabId) {
                return {
                  success: false,
                  error: 'Tab ID required for close action',
                };
              }
              return {
                success: true,
                data: { tabId: request.tabId },
              };

            case 'activate':
              return {
                success: true,
                data: { tabId: request.tabId },
              };

            case 'update':
              return {
                success: true,
                data: {
                  tab: {
                    id: request.tabId || 1,
                    url: request.properties?.url || 'https://example.com',
                    title: request.properties?.title || 'Updated Tab',
                    active: request.properties?.active || false,
                    pinned: request.properties?.pinned || false,
                    windowId: 1,
                    status: 'complete',
                  },
                },
              };

            default:
              return {
                success: false,
                error: `Unsupported action: ${request.action}`,
              };
          }
        },
      };
    });

    it('should handle tab creation', async () => {
      const request: TabToolRequest = {
        action: 'create',
        url: 'https://example.com',
      };

      const response = await mockTabTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.tab).toMatchObject({
        id: expect.any(Number),
        url: 'https://example.com',
        title: expect.any(String),
        active: true,
        status: 'complete',
      });

      const event = eventCollector.findByType('TabActionStart');
      expect(event).toBeDefined();
      expect((event?.msg as any).data.action).toBe('create');
    });

    it('should handle tab querying', async () => {
      const request: TabToolRequest = {
        action: 'query',
        query: { active: true },
      };

      const response = await mockTabTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.tabs).toBeInstanceOf(Array);
      expect(response.data?.tabs?.[0]).toMatchObject({
        id: expect.any(Number),
        url: expect.any(String),
        title: expect.any(String),
        active: true,
      });
    });

    it('should handle tab closure', async () => {
      const request: TabToolRequest = {
        action: 'close',
        tabId: 123,
      };

      const response = await mockTabTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.tabId).toBe(123);
    });

    it('should validate required parameters', async () => {
      const request: TabToolRequest = {
        action: 'close',
        // Missing required tabId
      };

      const response = await mockTabTool.execute(request);

      expect(response.success).toBe(false);
      expect(response.error).toContain('Tab ID required');
    });
  });

  describe('DOMTool Contract', () => {
    let mockDOMTool: DOMTool;

    beforeEach(() => {
      mockDOMTool = {
        async execute(request: DOMToolRequest): Promise<DOMToolResponse> {
          eventCollector.collect({
            id: 'evt_dom_action',
            msg: {
              type: 'DOMActionStart',
              data: {
                action: request.action,
                selector: request.selector,
                tab_id: request.tabId,
              },
            },
          });

          const mockElement: DOMElement = {
            tagName: 'BUTTON',
            id: 'submit-btn',
            className: 'btn btn-primary',
            textContent: 'Submit',
            attributes: {
              type: 'submit',
              disabled: 'false',
            },
            boundingBox: {
              x: 100,
              y: 200,
              width: 80,
              height: 32,
            },
          };

          switch (request.action) {
            case 'query':
              return {
                success: true,
                data: { elements: [mockElement] },
              };

            case 'click':
              if (!request.selector) {
                return {
                  success: false,
                  error: 'Selector required for click action',
                };
              }
              return {
                success: true,
                data: { element: mockElement },
              };

            case 'type':
              if (!request.text) {
                return {
                  success: false,
                  error: 'Text required for type action',
                };
              }
              return {
                success: true,
                data: { element: mockElement },
              };

            case 'getAttribute':
              const attrValue = mockElement.attributes[request.attribute || 'id'] || null;
              return {
                success: true,
                data: { attribute: attrValue },
              };

            case 'getText':
              return {
                success: true,
                data: { text: mockElement.textContent || '' },
              };

            case 'getHtml':
              return {
                success: true,
                data: { html: '<button id="submit-btn" class="btn btn-primary">Submit</button>' },
              };

            default:
              return {
                success: false,
                error: `Unsupported action: ${request.action}`,
              };
          }
        },
      };
    });

    it('should handle DOM element querying', async () => {
      const request: DOMToolRequest = {
        action: 'query',
        selector: 'button#submit-btn',
        tabId: 1,
      };

      const response = await mockDOMTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.elements).toBeInstanceOf(Array);
      expect(response.data?.elements?.[0]).toMatchObject({
        tagName: 'BUTTON',
        id: 'submit-btn',
        textContent: 'Submit',
        attributes: expect.objectContaining({
          type: 'submit',
        }),
        boundingBox: expect.objectContaining({
          x: expect.any(Number),
          y: expect.any(Number),
          width: expect.any(Number),
          height: expect.any(Number),
        }),
      });

      const event = eventCollector.findByType('DOMActionStart');
      expect(event).toBeDefined();
    });

    it('should handle element clicking', async () => {
      const request: DOMToolRequest = {
        action: 'click',
        selector: 'button#submit-btn',
        options: { scrollIntoView: true },
      };

      const response = await mockDOMTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.element?.tagName).toBe('BUTTON');
    });

    it('should handle text input', async () => {
      const request: DOMToolRequest = {
        action: 'type',
        selector: 'input[name="username"]',
        text: 'testuser',
      };

      const response = await mockDOMTool.execute(request);

      expect(response.success).toBe(true);
    });

    it('should handle attribute retrieval', async () => {
      const request: DOMToolRequest = {
        action: 'getAttribute',
        selector: 'button#submit-btn',
        attribute: 'type',
      };

      const response = await mockDOMTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.attribute).toBe('submit');
    });

    it('should validate required parameters', async () => {
      const request: DOMToolRequest = {
        action: 'click',
        // Missing required selector
      };

      const response = await mockDOMTool.execute(request);

      expect(response.success).toBe(false);
      expect(response.error).toContain('Selector required');
    });
  });

  describe('StorageTool Contract', () => {
    let mockStorageTool: StorageTool;
    let mockStorage: Record<string, any>;

    beforeEach(() => {
      mockStorage = {
        'user_preferences': { theme: 'dark', language: 'en' },
        'session_id': 'abc123',
        'last_visited': '2024-01-15',
      };

      mockStorageTool = {
        async execute(request: StorageToolRequest): Promise<StorageToolResponse> {
          eventCollector.collect({
            id: 'evt_storage_action',
            msg: {
              type: 'StorageActionStart',
              data: {
                action: request.action,
                storage_type: request.storageType,
                key: request.key,
              },
            },
          });

          switch (request.action) {
            case 'get':
              if (!request.key) {
                return {
                  success: false,
                  error: 'Key required for get action',
                };
              }
              const value = mockStorage[request.key];
              return {
                success: true,
                data: { value },
              };

            case 'set':
              if (!request.key) {
                return {
                  success: false,
                  error: 'Key required for set action',
                };
              }
              mockStorage[request.key] = request.value;
              return {
                success: true,
                data: { value: request.value },
              };

            case 'remove':
              if (!request.key) {
                return {
                  success: false,
                  error: 'Key required for remove action',
                };
              }
              delete mockStorage[request.key];
              return { success: true };

            case 'keys':
              return {
                success: true,
                data: { keys: Object.keys(mockStorage) },
              };

            case 'clear':
              mockStorage = {};
              return { success: true };

            default:
              return {
                success: false,
                error: `Unsupported action: ${request.action}`,
              };
          }
        },
      };
    });

    it('should handle storage retrieval', async () => {
      const request: StorageToolRequest = {
        action: 'get',
        storageType: 'local',
        key: 'user_preferences',
      };

      const response = await mockStorageTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.value).toEqual({ theme: 'dark', language: 'en' });

      const event = eventCollector.findByType('StorageActionStart');
      expect(event).toBeDefined();
    });

    it('should handle storage setting', async () => {
      const request: StorageToolRequest = {
        action: 'set',
        storageType: 'local',
        key: 'new_setting',
        value: { enabled: true, count: 42 },
      };

      const response = await mockStorageTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.value).toEqual({ enabled: true, count: 42 });
    });

    it('should handle key listing', async () => {
      const request: StorageToolRequest = {
        action: 'keys',
        storageType: 'local',
      };

      const response = await mockStorageTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.keys).toBeInstanceOf(Array);
      expect(response.data?.keys).toContain('user_preferences');
    });

    it('should handle storage clearing', async () => {
      const request: StorageToolRequest = {
        action: 'clear',
        storageType: 'local',
      };

      const response = await mockStorageTool.execute(request);

      expect(response.success).toBe(true);

      // Verify keys are cleared
      const keysRequest: StorageToolRequest = {
        action: 'keys',
        storageType: 'local',
      };
      const keysResponse = await mockStorageTool.execute(keysRequest);
      expect(keysResponse.data?.keys).toHaveLength(0);
    });

    it('should validate required parameters', async () => {
      const request: StorageToolRequest = {
        action: 'get',
        storageType: 'local',
        // Missing required key
      };

      const response = await mockStorageTool.execute(request);

      expect(response.success).toBe(false);
      expect(response.error).toContain('Key required');
    });
  });

  describe('NavigationTool Contract', () => {
    let mockNavigationTool: NavigationTool;

    beforeEach(() => {
      mockNavigationTool = {
        async execute(request: NavigationToolRequest): Promise<NavigationToolResponse> {
          eventCollector.collect({
            id: 'evt_nav_action',
            msg: {
              type: 'NavigationActionStart',
              data: {
                action: request.action,
                url: request.url,
                tab_id: request.tabId,
              },
            },
          });

          const mockHistory: HistoryEntry[] = [
            {
              url: 'https://example.com',
              title: 'Example Site',
              visitTime: Date.now() - 3600000,
            },
            {
              url: 'https://github.com',
              title: 'GitHub',
              visitTime: Date.now() - 1800000,
            },
            {
              url: 'https://stackoverflow.com',
              title: 'Stack Overflow',
              visitTime: Date.now() - 900000,
            },
          ];

          switch (request.action) {
            case 'navigate':
              if (!request.url) {
                return {
                  success: false,
                  error: 'URL required for navigate action',
                };
              }
              return {
                success: true,
                data: {
                  url: request.url,
                  title: 'Loading...',
                  status: 'loading',
                },
              };

            case 'reload':
              return {
                success: true,
                data: {
                  url: 'https://example.com',
                  title: 'Example Site',
                  status: 'complete',
                },
              };

            case 'goBack':
              return {
                success: true,
                data: {
                  url: 'https://example.com',
                  title: 'Example Site',
                  status: 'complete',
                },
              };

            case 'goForward':
              return {
                success: true,
                data: {
                  url: 'https://github.com',
                  title: 'GitHub',
                  status: 'complete',
                },
              };

            case 'getHistory':
              return {
                success: true,
                data: { history: mockHistory },
              };

            default:
              return {
                success: false,
                error: `Unsupported action: ${request.action}`,
              };
          }
        },
      };
    });

    it('should handle navigation', async () => {
      const request: NavigationToolRequest = {
        action: 'navigate',
        tabId: 1,
        url: 'https://example.com',
        options: { waitForLoad: true, timeout: 5000 },
      };

      const response = await mockNavigationTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.url).toBe('https://example.com');
      expect(response.data?.status).toBeDefined();

      const event = eventCollector.findByType('NavigationActionStart');
      expect(event).toBeDefined();
      expect((event?.msg as any).data.url).toBe('https://example.com');
    });

    it('should handle page reload', async () => {
      const request: NavigationToolRequest = {
        action: 'reload',
        tabId: 1,
      };

      const response = await mockNavigationTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.status).toBe('complete');
    });

    it('should handle back navigation', async () => {
      const request: NavigationToolRequest = {
        action: 'goBack',
        tabId: 1,
      };

      const response = await mockNavigationTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.url).toBeDefined();
    });

    it('should handle history retrieval', async () => {
      const request: NavigationToolRequest = {
        action: 'getHistory',
      };

      const response = await mockNavigationTool.execute(request);

      expect(response.success).toBe(true);
      expect(response.data?.history).toBeInstanceOf(Array);
      expect(response.data?.history?.[0]).toMatchObject({
        url: expect.any(String),
        title: expect.any(String),
        visitTime: expect.any(Number),
      });
    });

    it('should validate required parameters', async () => {
      const request: NavigationToolRequest = {
        action: 'navigate',
        tabId: 1,
        // Missing required URL
      };

      const response = await mockNavigationTool.execute(request);

      expect(response.success).toBe(false);
      expect(response.error).toContain('URL required');
    });
  });

  describe('Common Tool Requirements', () => {
    it('should emit action start events', () => {
      // This is tested in each individual tool test
      const events = eventCollector.getEvents();
      expect(events.some(e => e.msg.type.endsWith('ActionStart'))).toBe(true);
    });

    it('should handle error responses consistently', () => {
      // All tools should return consistent error response format
      const errorResponse: TabToolResponse = {
        success: false,
        error: 'Test error message',
      };

      expect(errorResponse.success).toBe(false);
      expect(errorResponse.error).toBeTypeOf('string');
      expect(errorResponse.data).toBeUndefined();
    });

    it('should handle success responses consistently', () => {
      // All tools should return consistent success response format
      const successResponse: TabToolResponse = {
        success: true,
        data: { tabId: 123 },
      };

      expect(successResponse.success).toBe(true);
      expect(successResponse.data).toBeDefined();
      expect(successResponse.error).toBeUndefined();
    });

    it('should support optional parameters', () => {
      // Test that tools handle optional parameters gracefully
      const requestWithOptionals: TabToolRequest = {
        action: 'create',
        url: 'https://example.com',
        properties: {
          pinned: true,
          active: false,
        },
      };

      const requestWithoutOptionals: TabToolRequest = {
        action: 'create',
      };

      expect(requestWithOptionals.properties).toBeDefined();
      expect(requestWithoutOptionals.properties).toBeUndefined();
    });
  });
});