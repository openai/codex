/**
 * Browser Tools System - Index
 *
 * Exports all browser tools and utilities for the codex-chrome extension.
 * Provides a centralized entry point for tool registration and management.
 */

// Core tool system exports
export { ToolRegistry, toolRegistry } from './ToolRegistry';
export type {
  ToolDefinition,
  ToolParameterSchema,
  ParameterProperty,
  ToolExecutionRequest,
  ToolExecutionResponse,
  ToolError,
  ToolDiscoveryQuery,
  ToolDiscoveryResult,
  ToolValidationResult,
  ValidationError,
  ToolContext,
  ToolHandler,
} from './ToolRegistry';

// Base tool exports
export { BaseTool, createToolDefinition } from './BaseTool';
export type {
  BaseToolRequest,
  BaseToolOptions,
  ToolResult,
} from './BaseTool';

// Individual tool exports
export { TabTool } from './TabTool';
export type {
  TabToolRequest,
  TabProperties,
  TabQuery,
  ScreenshotOptions,
  TabInfo,
  TabToolResponse,
} from './TabTool';

export { DOMTool } from './DOMTool';
export type {
  DOMToolRequest,
  DOMActionOptions,
  DOMElement,
  BoundingBox,
  DOMToolResponse,
} from './DOMTool';

export { StorageTool } from './StorageTool';
export type {
  StorageToolRequest,
  StorageOptions,
  StorageToolResponse,
  StorageQuota,
} from './StorageTool';

export { NavigationTool } from './NavigationTool';
export type {
  NavigationToolRequest,
  NavigationOptions,
  NavigationToolResponse,
  HistoryEntry,
  NavigationError,
  NavigationEvent,
} from './NavigationTool';

/**
 * Create and configure a complete tool registry with all browser tools
 */
export async function createBrowserToolRegistry(): Promise<ToolRegistry> {
  const registry = new ToolRegistry();

  // Create tool instances
  const tabTool = new TabTool();
  const domTool = new DOMTool();
  const storageTool = new StorageTool();
  const navigationTool = new NavigationTool();

  // Register all tools
  await registry.register(tabTool.getDefinition(), async (params, context) => {
    return tabTool.execute(params);
  });

  await registry.register(domTool.getDefinition(), async (params, context) => {
    return domTool.execute(params);
  });

  await registry.register(storageTool.getDefinition(), async (params, context) => {
    return storageTool.execute(params);
  });

  await registry.register(navigationTool.getDefinition(), async (params, context) => {
    return navigationTool.execute(params);
  });

  return registry;
}

/**
 * Tool categories for organization
 */
export const TOOL_CATEGORIES = {
  BROWSER: 'browser',
  DOM: 'dom',
  STORAGE: 'storage',
  NAVIGATION: 'navigation',
} as const;

/**
 * Default tool configurations
 */
export const DEFAULT_TOOL_OPTIONS = {
  timeout: 30000,
  retries: 3,
  waitForLoad: true,
} as const;

/**
 * Common tool error codes
 */
export const TOOL_ERROR_CODES = {
  // General errors
  EXECUTION_ERROR: 'EXECUTION_ERROR',
  TIMEOUT: 'TIMEOUT',
  VALIDATION_ERROR: 'VALIDATION_ERROR',

  // Permission errors
  PERMISSION_DENIED: 'PERMISSION_DENIED',
  CHROME_API_UNAVAILABLE: 'CHROME_API_UNAVAILABLE',

  // Tab errors
  TAB_NOT_FOUND: 'TAB_NOT_FOUND',
  TAB_CLOSED: 'TAB_CLOSED',

  // DOM errors
  ELEMENT_NOT_FOUND: 'ELEMENT_NOT_FOUND',
  CONTENT_SCRIPT_ERROR: 'CONTENT_SCRIPT_ERROR',

  // Storage errors
  STORAGE_QUOTA_EXCEEDED: 'STORAGE_QUOTA_EXCEEDED',
  STORAGE_KEY_NOT_FOUND: 'STORAGE_KEY_NOT_FOUND',

  // Navigation errors
  NAVIGATION_FAILED: 'NAVIGATION_FAILED',
  INVALID_URL: 'INVALID_URL',
  LOAD_TIMEOUT: 'LOAD_TIMEOUT',
} as const;

/**
 * Utility function to check if Chrome extension APIs are available
 */
export function isChromeExtensionContext(): boolean {
  return typeof chrome !== 'undefined' && !!chrome.runtime && !!chrome.runtime.id;
}

/**
 * Utility function to check for specific permissions
 */
export async function checkPermissions(permissions: string[]): Promise<boolean> {
  if (!isChromeExtensionContext() || !chrome.permissions) {
    return false;
  }

  try {
    return await chrome.permissions.contains({ permissions });
  } catch (error) {
    console.warn('Permission check failed:', error);
    return false;
  }
}

/**
 * Utility function to request permissions
 */
export async function requestPermissions(permissions: string[]): Promise<boolean> {
  if (!isChromeExtensionContext() || !chrome.permissions) {
    return false;
  }

  try {
    return await chrome.permissions.request({ permissions });
  } catch (error) {
    console.warn('Permission request failed:', error);
    return false;
  }
}

/**
 * Get tool statistics from registry
 */
export function getToolStats(registry: ToolRegistry) {
  const stats = registry.getStats();
  const tools = registry.listTools();

  const toolsByCategory = tools.reduce((acc, tool) => {
    const category = tool.category || 'uncategorized';
    acc[category] = (acc[category] || 0) + 1;
    return acc;
  }, {} as Record<string, number>);

  return {
    ...stats,
    toolsByCategory,
    averageToolsPerCategory: stats.totalTools / stats.categories.length,
  };
}

/**
 * Validate tool compatibility with current browser context
 */
export async function validateToolCompatibility(): Promise<{
  compatible: boolean;
  missingAPIs: string[];
  missingPermissions: string[];
  warnings: string[];
}> {
  const missingAPIs: string[] = [];
  const missingPermissions: string[] = [];
  const warnings: string[] = [];

  // Check Chrome APIs
  if (!chrome?.tabs) missingAPIs.push('tabs');
  if (!chrome?.storage) missingAPIs.push('storage');
  if (!chrome?.scripting) missingAPIs.push('scripting');
  if (!chrome?.webNavigation) warnings.push('webNavigation API not available - some navigation features may be limited');

  // Check permissions
  const requiredPermissions = ['tabs', 'storage', 'activeTab', 'scripting'];
  for (const permission of requiredPermissions) {
    const hasPermission = await checkPermissions([permission]);
    if (!hasPermission) {
      missingPermissions.push(permission);
    }
  }

  return {
    compatible: missingAPIs.length === 0 && missingPermissions.length === 0,
    missingAPIs,
    missingPermissions,
    warnings,
  };
}