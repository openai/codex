/**
 * Tab Tool
 *
 * Provides browser tab management capabilities including creating, closing,
 * switching, querying tabs, and taking screenshots.
 */

import { BaseTool, BaseToolRequest, BaseToolOptions, createToolDefinition } from './BaseTool';
import { ToolDefinition } from './ToolRegistry';

/**
 * Tab tool request interface
 */
export interface TabToolRequest extends BaseToolRequest {
  action: 'create' | 'close' | 'update' | 'query' | 'activate' | 'screenshot' | 'duplicate';
  tabId?: number;
  url?: string;
  properties?: TabProperties;
  query?: TabQuery;
  screenshotOptions?: ScreenshotOptions;
}

/**
 * Tab properties for updates
 */
export interface TabProperties {
  url?: string;
  title?: string;
  active?: boolean;
  pinned?: boolean;
  muted?: boolean;
  windowId?: number;
}

/**
 * Tab query criteria
 */
export interface TabQuery {
  url?: string;
  title?: string;
  active?: boolean;
  windowId?: number;
  pinned?: boolean;
  muted?: boolean;
  status?: 'loading' | 'complete';
}

/**
 * Screenshot options
 */
export interface ScreenshotOptions {
  format?: 'jpeg' | 'png';
  quality?: number;
  fromSurface?: boolean;
}

/**
 * Tab information
 */
export interface TabInfo {
  id: number;
  url: string;
  title: string;
  active: boolean;
  pinned: boolean;
  muted?: boolean;
  windowId: number;
  status: 'loading' | 'complete';
  index: number;
  favicon?: string;
  incognito?: boolean;
}

/**
 * Tab tool response data
 */
export interface TabToolResponse {
  tabs?: TabInfo[];
  tab?: TabInfo;
  tabId?: number;
  screenshot?: string; // Base64 encoded image
  duplicatedTab?: TabInfo;
}

/**
 * Tab Tool Implementation
 *
 * Handles all browser tab operations using Chrome extension APIs.
 */
export class TabTool extends BaseTool {
  protected toolDefinition: ToolDefinition = createToolDefinition(
    'browser_tab',
    'Manage browser tabs - create, close, update, query, activate, and screenshot',
    {
      action: {
        type: 'string',
        description: 'The action to perform on tabs',
        enum: ['create', 'close', 'update', 'query', 'activate', 'screenshot', 'duplicate'],
      },
      tabId: {
        type: 'number',
        description: 'Tab ID for operations on specific tabs',
      },
      url: {
        type: 'string',
        description: 'URL for tab creation or navigation',
      },
      properties: {
        type: 'object',
        description: 'Tab properties for updates',
        properties: {
          url: { type: 'string', description: 'Tab URL' },
          title: { type: 'string', description: 'Tab title' },
          active: { type: 'boolean', description: 'Whether tab should be active' },
          pinned: { type: 'boolean', description: 'Whether tab should be pinned' },
          muted: { type: 'boolean', description: 'Whether tab should be muted' },
          windowId: { type: 'number', description: 'Window ID to move tab to' },
        },
      },
      query: {
        type: 'object',
        description: 'Query criteria for finding tabs',
        properties: {
          url: { type: 'string', description: 'URL pattern to match' },
          title: { type: 'string', description: 'Title pattern to match' },
          active: { type: 'boolean', description: 'Filter by active status' },
          windowId: { type: 'number', description: 'Filter by window ID' },
          pinned: { type: 'boolean', description: 'Filter by pinned status' },
          muted: { type: 'boolean', description: 'Filter by muted status' },
          status: { type: 'string', enum: ['loading', 'complete'], description: 'Filter by loading status' },
        },
      },
      screenshotOptions: {
        type: 'object',
        description: 'Options for taking screenshots',
        properties: {
          format: { type: 'string', enum: ['jpeg', 'png'], default: 'png', description: 'Screenshot format' },
          quality: { type: 'number', description: 'JPEG quality (0-100)' },
          fromSurface: { type: 'boolean', default: false, description: 'Capture from surface instead of DOM' },
        },
      },
    },
    {
      required: ['action'],
      category: 'browser',
      version: '1.0.0',
      metadata: {
        capabilities: ['tab_management', 'screenshot', 'navigation'],
        permissions: ['tabs', 'activeTab'],
      },
    }
  );

  /**
   * Execute tab tool action
   */
  protected async executeImpl(request: TabToolRequest, options?: BaseToolOptions): Promise<TabToolResponse> {
    // Validate Chrome context
    this.validateChromeContext();

    // Validate required permissions
    await this.validatePermissions(['tabs']);

    this.log('debug', `Executing tab action: ${request.action}`, request);

    switch (request.action) {
      case 'create':
        return this.createTab(request);

      case 'close':
        return this.closeTab(request);

      case 'update':
        return this.updateTab(request);

      case 'query':
        return this.queryTabs(request);

      case 'activate':
        return this.activateTab(request);

      case 'screenshot':
        return this.takeScreenshot(request);

      case 'duplicate':
        return this.duplicateTab(request);

      default:
        throw new Error(`Unsupported action: ${request.action}`);
    }
  }

  /**
   * Create a new tab
   */
  private async createTab(request: TabToolRequest): Promise<TabToolResponse> {
    const createProperties: chrome.tabs.CreateProperties = {
      url: request.url || 'about:blank',
      active: request.properties?.active ?? true,
      pinned: request.properties?.pinned ?? false,
      windowId: request.properties?.windowId,
    };

    try {
      const tab = await chrome.tabs.create(createProperties);

      // Wait for tab to load if URL was provided
      if (request.url && request.url !== 'about:blank') {
        await this.waitForTabToLoad(tab.id!);
      }

      const tabInfo = this.convertTabToInfo(tab);

      this.log('info', `Created tab with ID: ${tab.id}`, tabInfo);

      return {
        tab: tabInfo,
        tabId: tab.id,
      };
    } catch (error) {
      throw new Error(`Failed to create tab: ${error}`);
    }
  }

  /**
   * Close a tab
   */
  private async closeTab(request: TabToolRequest): Promise<TabToolResponse> {
    if (!request.tabId) {
      throw new Error('Tab ID is required for close action');
    }

    try {
      // Validate tab exists before closing
      await this.validateTabId(request.tabId);

      await chrome.tabs.remove(request.tabId);

      this.log('info', `Closed tab with ID: ${request.tabId}`);

      return {
        tabId: request.tabId,
      };
    } catch (error) {
      throw new Error(`Failed to close tab ${request.tabId}: ${error}`);
    }
  }

  /**
   * Update tab properties
   */
  private async updateTab(request: TabToolRequest): Promise<TabToolResponse> {
    if (!request.tabId) {
      throw new Error('Tab ID is required for update action');
    }

    if (!request.properties) {
      throw new Error('Properties are required for update action');
    }

    try {
      // Validate tab exists
      await this.validateTabId(request.tabId);

      const updateProperties: chrome.tabs.UpdateProperties = {
        url: request.properties.url,
        active: request.properties.active,
        pinned: request.properties.pinned,
        muted: request.properties.muted,
      };

      // Remove undefined properties
      Object.keys(updateProperties).forEach(key => {
        if (updateProperties[key as keyof chrome.tabs.UpdateProperties] === undefined) {
          delete updateProperties[key as keyof chrome.tabs.UpdateProperties];
        }
      });

      const updatedTab = await chrome.tabs.update(request.tabId, updateProperties);

      // Handle window movement if specified
      if (request.properties.windowId && updatedTab.windowId !== request.properties.windowId) {
        await chrome.tabs.move(request.tabId, {
          windowId: request.properties.windowId,
          index: -1,
        });
      }

      // Get the final tab state
      const finalTab = await chrome.tabs.get(request.tabId);
      const tabInfo = this.convertTabToInfo(finalTab);

      this.log('info', `Updated tab ${request.tabId}`, tabInfo);

      return {
        tab: tabInfo,
        tabId: request.tabId,
      };
    } catch (error) {
      throw new Error(`Failed to update tab ${request.tabId}: ${error}`);
    }
  }

  /**
   * Query tabs based on criteria
   */
  private async queryTabs(request: TabToolRequest): Promise<TabToolResponse> {
    try {
      const queryInfo: chrome.tabs.QueryInfo = {};

      if (request.query) {
        if (request.query.url) queryInfo.url = request.query.url;
        if (request.query.title) queryInfo.title = request.query.title;
        if (request.query.active !== undefined) queryInfo.active = request.query.active;
        if (request.query.windowId) queryInfo.windowId = request.query.windowId;
        if (request.query.pinned !== undefined) queryInfo.pinned = request.query.pinned;
        if (request.query.muted !== undefined) queryInfo.muted = request.query.muted;
        if (request.query.status) queryInfo.status = request.query.status;
      }

      const tabs = await chrome.tabs.query(queryInfo);
      const tabInfos = tabs.map(tab => this.convertTabToInfo(tab));

      this.log('info', `Found ${tabInfos.length} tabs matching query`, request.query);

      return {
        tabs: tabInfos,
      };
    } catch (error) {
      throw new Error(`Failed to query tabs: ${error}`);
    }
  }

  /**
   * Activate a specific tab
   */
  private async activateTab(request: TabToolRequest): Promise<TabToolResponse> {
    if (!request.tabId) {
      throw new Error('Tab ID is required for activate action');
    }

    try {
      // Validate tab exists
      const tab = await this.validateTabId(request.tabId);

      // Update tab to be active
      await chrome.tabs.update(request.tabId, { active: true });

      // Focus the window containing the tab
      if (tab.windowId) {
        await chrome.windows.update(tab.windowId, { focused: true });
      }

      const tabInfo = this.convertTabToInfo(tab);

      this.log('info', `Activated tab ${request.tabId}`);

      return {
        tab: tabInfo,
        tabId: request.tabId,
      };
    } catch (error) {
      throw new Error(`Failed to activate tab ${request.tabId}: ${error}`);
    }
  }

  /**
   * Take a screenshot of a tab
   */
  private async takeScreenshot(request: TabToolRequest): Promise<TabToolResponse> {
    const tabId = request.tabId;

    try {
      // If no tab ID provided, use active tab
      const targetTab = tabId ? await this.validateTabId(tabId) : await this.getActiveTab();

      // Validate screenshot permissions
      await this.validatePermissions(['activeTab']);

      const options: chrome.tabs.CaptureVisibleTabOptions = {
        format: request.screenshotOptions?.format || 'png',
        quality: request.screenshotOptions?.quality,
      };

      // Remove undefined properties
      if (options.quality === undefined) {
        delete options.quality;
      }

      // Ensure tab is active for screenshot
      if (!targetTab.active) {
        await chrome.tabs.update(targetTab.id!, { active: true });
        // Wait a moment for the tab to become fully active
        await new Promise(resolve => setTimeout(resolve, 500));
      }

      const screenshotDataUrl = await chrome.tabs.captureVisibleTab(
        targetTab.windowId,
        options
      );

      const tabInfo = this.convertTabToInfo(targetTab);

      this.log('info', `Captured screenshot of tab ${targetTab.id}`);

      return {
        tab: tabInfo,
        tabId: targetTab.id,
        screenshot: screenshotDataUrl,
      };
    } catch (error) {
      throw new Error(`Failed to take screenshot: ${error}`);
    }
  }

  /**
   * Duplicate a tab
   */
  private async duplicateTab(request: TabToolRequest): Promise<TabToolResponse> {
    if (!request.tabId) {
      throw new Error('Tab ID is required for duplicate action');
    }

    try {
      const duplicatedTab = await chrome.tabs.duplicate(request.tabId);
      const tabInfo = this.convertTabToInfo(duplicatedTab);

      this.log('info', `Duplicated tab ${request.tabId} as ${duplicatedTab.id}`);

      return {
        duplicatedTab: tabInfo,
        tabId: duplicatedTab.id,
      };
    } catch (error) {
      throw new Error(`Failed to duplicate tab ${request.tabId}: ${error}`);
    }
  }

  /**
   * Convert Chrome tab to TabInfo
   */
  private convertTabToInfo(tab: chrome.tabs.Tab): TabInfo {
    return {
      id: tab.id!,
      url: tab.url || '',
      title: tab.title || '',
      active: tab.active || false,
      pinned: tab.pinned || false,
      muted: tab.mutedInfo?.muted || false,
      windowId: tab.windowId!,
      status: tab.status as 'loading' | 'complete' || 'complete',
      index: tab.index,
      favicon: tab.favIconUrl,
      incognito: tab.incognito,
    };
  }

  /**
   * Wait for tab to finish loading
   */
  private async waitForTabToLoad(tabId: number, timeoutMs: number = 10000): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeoutMs) {
      const tab = await chrome.tabs.get(tabId);

      if (tab.status === 'complete') {
        return;
      }

      // Wait a bit before checking again
      await new Promise(resolve => setTimeout(resolve, 100));
    }

    this.log('warn', `Tab ${tabId} did not finish loading within ${timeoutMs}ms`);
  }

  /**
   * Get all tabs in all windows
   */
  async getAllTabs(): Promise<TabInfo[]> {
    try {
      const tabs = await chrome.tabs.query({});
      return tabs.map(tab => this.convertTabToInfo(tab));
    } catch (error) {
      throw new Error(`Failed to get all tabs: ${error}`);
    }
  }

  /**
   * Get current active tab
   */
  async getCurrentTab(): Promise<TabInfo> {
    try {
      const tab = await this.getActiveTab();
      return this.convertTabToInfo(tab);
    } catch (error) {
      throw new Error(`Failed to get current tab: ${error}`);
    }
  }

  /**
   * Close multiple tabs by IDs
   */
  async closeTabs(tabIds: number[]): Promise<void> {
    try {
      await chrome.tabs.remove(tabIds);
      this.log('info', `Closed ${tabIds.length} tabs`);
    } catch (error) {
      throw new Error(`Failed to close multiple tabs: ${error}`);
    }
  }

  /**
   * Move tab to different position/window
   */
  async moveTab(tabId: number, moveProperties: chrome.tabs.MoveProperties): Promise<TabInfo> {
    try {
      const movedTabs = await chrome.tabs.move(tabId, moveProperties);
      const movedTab = Array.isArray(movedTabs) ? movedTabs[0] : movedTabs;
      return this.convertTabToInfo(movedTab);
    } catch (error) {
      throw new Error(`Failed to move tab ${tabId}: ${error}`);
    }
  }

  /**
   * Listen for tab events
   */
  setupTabEventListeners(callback: (eventType: string, tabInfo?: TabInfo) => void): void {
    chrome.tabs.onCreated.addListener((tab) => {
      callback('created', this.convertTabToInfo(tab));
    });

    chrome.tabs.onRemoved.addListener((tabId, removeInfo) => {
      callback('removed', { id: tabId } as TabInfo);
    });

    chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
      if (changeInfo.status === 'complete') {
        callback('updated', this.convertTabToInfo(tab));
      }
    });

    chrome.tabs.onActivated.addListener((activeInfo) => {
      chrome.tabs.get(activeInfo.tabId).then(tab => {
        callback('activated', this.convertTabToInfo(tab));
      });
    });
  }
}