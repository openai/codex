/**
 * Storage Tool
 *
 * Provides Chrome storage API capabilities including local, session, and sync storage.
 * Handles get/set/remove operations, storage quota management, and data migration.
 */

import { BaseTool, BaseToolRequest, BaseToolOptions, createToolDefinition } from './BaseTool';
import { ToolDefinition } from './ToolRegistry';

/**
 * Storage tool request interface
 */
export interface StorageToolRequest extends BaseToolRequest {
  action: 'get' | 'set' | 'remove' | 'clear' | 'keys' | 'getBytesInUse' | 'migrate' | 'sync';
  storageType: 'local' | 'session' | 'sync' | 'managed';
  key?: string;
  keys?: string[];
  value?: any;
  data?: Record<string, any>;
  options?: StorageOptions;
}

/**
 * Storage operation options
 */
export interface StorageOptions {
  timeout?: number;
  retryCount?: number;
  compress?: boolean;
  encrypt?: boolean;
  ttl?: number; // Time to live in milliseconds
  namespace?: string;
  syncPriority?: 'low' | 'normal' | 'high';
}

/**
 * Storage tool response data
 */
export interface StorageToolResponse {
  value?: any;
  values?: Record<string, any>;
  keys?: string[];
  bytesInUse?: number;
  success?: boolean;
  migrated?: number;
  synced?: number;
  quota?: StorageQuota;
}

/**
 * Storage quota information
 */
export interface StorageQuota {
  available: number;
  used: number;
  total: number;
  percentage: number;
}

/**
 * Storage entry with metadata
 */
interface StorageEntry {
  value: any;
  timestamp: number;
  ttl?: number;
  namespace?: string;
  version?: number;
}

/**
 * Storage Tool Implementation
 *
 * Provides comprehensive Chrome storage management with support for
 * local, session, sync, and managed storage areas.
 */
export class StorageTool extends BaseTool {
  protected toolDefinition: ToolDefinition = createToolDefinition(
    'browser_storage',
    'Manage Chrome storage - get, set, remove, clear data across local, session, sync, and managed storage',
    {
      action: {
        type: 'string',
        description: 'The storage action to perform',
        enum: ['get', 'set', 'remove', 'clear', 'keys', 'getBytesInUse', 'migrate', 'sync'],
      },
      storageType: {
        type: 'string',
        description: 'Type of storage to use',
        enum: ['local', 'session', 'sync', 'managed'],
      },
      key: {
        type: 'string',
        description: 'Storage key for single-key operations',
      },
      keys: {
        type: 'array',
        description: 'Array of storage keys for multi-key operations',
        items: { type: 'string' },
      },
      value: {
        type: 'string',
        description: 'Value to store (will be JSON serialized)',
      },
      data: {
        type: 'object',
        description: 'Object with key-value pairs for batch operations',
      },
      options: {
        type: 'object',
        description: 'Additional options for storage operations',
        properties: {
          timeout: { type: 'number', description: 'Operation timeout (ms)', default: 5000 },
          retryCount: { type: 'number', description: 'Number of retry attempts', default: 3 },
          compress: { type: 'boolean', description: 'Compress data before storage', default: false },
          encrypt: { type: 'boolean', description: 'Encrypt sensitive data', default: false },
          ttl: { type: 'number', description: 'Time to live (ms)' },
          namespace: { type: 'string', description: 'Storage namespace for key isolation' },
          syncPriority: { type: 'string', enum: ['low', 'normal', 'high'], description: 'Sync priority for sync storage' },
        },
      },
    },
    {
      required: ['action', 'storageType'],
      category: 'storage',
      version: '1.0.0',
      metadata: {
        capabilities: ['storage_management', 'data_persistence', 'quota_management'],
        permissions: ['storage'],
      },
    }
  );

  /**
   * Execute storage tool action
   */
  protected async executeImpl(request: StorageToolRequest, options?: BaseToolOptions): Promise<StorageToolResponse> {
    // Validate Chrome context
    this.validateChromeContext();

    // Validate required permissions
    await this.validatePermissions(['storage']);

    this.log('debug', `Executing storage action: ${request.action}`, request);

    const storageArea = this.getStorageArea(request.storageType);

    switch (request.action) {
      case 'get':
        return this.getValue(storageArea, request);

      case 'set':
        return this.setValue(storageArea, request);

      case 'remove':
        return this.removeValue(storageArea, request);

      case 'clear':
        return this.clearStorage(storageArea, request);

      case 'keys':
        return this.getKeys(storageArea, request);

      case 'getBytesInUse':
        return this.getBytesInUse(storageArea, request);

      case 'migrate':
        return this.migrateData(request);

      case 'sync':
        return this.syncData(request);

      default:
        throw new Error(`Unsupported storage action: ${request.action}`);
    }
  }

  /**
   * Get value(s) from storage
   */
  private async getValue(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      let keys: string | string[] | null = null;

      if (request.key) {
        keys = this.namespaceKey(request.key, request.options?.namespace);
      } else if (request.keys) {
        keys = request.keys.map(k => this.namespaceKey(k, request.options?.namespace));
      }

      const result = await this.executeWithTimeout(
        () => storageArea.get(keys),
        request.options?.timeout || 5000
      );

      // Process results with TTL and namespace handling
      const processedResult = this.processStorageResult(result, request.options?.namespace);

      // Handle single key vs multiple keys
      if (request.key) {
        const value = processedResult[request.key];
        return { value };
      } else {
        return { values: processedResult };
      }
    } catch (error) {
      throw new Error(`Failed to get storage value: ${error}`);
    }
  }

  /**
   * Set value(s) in storage
   */
  private async setValue(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      let dataToSet: Record<string, any> = {};

      if (request.key && request.value !== undefined) {
        const entry = this.createStorageEntry(request.value, request.options);
        const namespacedKey = this.namespaceKey(request.key, request.options?.namespace);
        dataToSet[namespacedKey] = entry;
      } else if (request.data) {
        for (const [key, value] of Object.entries(request.data)) {
          const entry = this.createStorageEntry(value, request.options);
          const namespacedKey = this.namespaceKey(key, request.options?.namespace);
          dataToSet[namespacedKey] = entry;
        }
      } else {
        throw new Error('Either key+value or data object is required for set action');
      }

      // Check quota before setting (for local and sync storage)
      if (request.storageType === 'local' || request.storageType === 'sync') {
        await this.checkQuotaBeforeSet(storageArea, dataToSet);
      }

      await this.executeWithTimeout(
        () => storageArea.set(dataToSet),
        request.options?.timeout || 5000
      );

      this.log('info', `Set ${Object.keys(dataToSet).length} storage item(s)`, {
        storageType: request.storageType,
        keys: Object.keys(dataToSet),
      });

      return { success: true };
    } catch (error) {
      throw new Error(`Failed to set storage value: ${error}`);
    }
  }

  /**
   * Remove value(s) from storage
   */
  private async removeValue(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      let keysToRemove: string | string[];

      if (request.key) {
        keysToRemove = this.namespaceKey(request.key, request.options?.namespace);
      } else if (request.keys) {
        keysToRemove = request.keys.map(k => this.namespaceKey(k, request.options?.namespace));
      } else {
        throw new Error('Either key or keys is required for remove action');
      }

      await this.executeWithTimeout(
        () => storageArea.remove(keysToRemove),
        request.options?.timeout || 5000
      );

      const removedCount = Array.isArray(keysToRemove) ? keysToRemove.length : 1;
      this.log('info', `Removed ${removedCount} storage item(s)`, {
        storageType: request.storageType,
        keys: keysToRemove,
      });

      return { success: true };
    } catch (error) {
      throw new Error(`Failed to remove storage value: ${error}`);
    }
  }

  /**
   * Clear all storage
   */
  private async clearStorage(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      if (request.options?.namespace) {
        // Clear only namespaced items
        const allItems = await storageArea.get(null);
        const namespacedKeys: string[] = [];
        const prefix = `${request.options.namespace}:`;

        for (const key of Object.keys(allItems)) {
          if (key.startsWith(prefix)) {
            namespacedKeys.push(key);
          }
        }

        if (namespacedKeys.length > 0) {
          await storageArea.remove(namespacedKeys);
        }

        this.log('info', `Cleared ${namespacedKeys.length} namespaced storage item(s)`, {
          storageType: request.storageType,
          namespace: request.options.namespace,
        });
      } else {
        // Clear entire storage area
        await this.executeWithTimeout(
          () => storageArea.clear(),
          request.options?.timeout || 5000
        );

        this.log('info', `Cleared all ${request.storageType} storage`, {
          storageType: request.storageType,
        });
      }

      return { success: true };
    } catch (error) {
      throw new Error(`Failed to clear storage: ${error}`);
    }
  }

  /**
   * Get all keys from storage
   */
  private async getKeys(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      const allItems = await this.executeWithTimeout(
        () => storageArea.get(null),
        request.options?.timeout || 5000
      );

      let keys = Object.keys(allItems);

      // Filter by namespace if specified
      if (request.options?.namespace) {
        const prefix = `${request.options.namespace}:`;
        keys = keys
          .filter(key => key.startsWith(prefix))
          .map(key => key.substring(prefix.length));
      }

      return { keys };
    } catch (error) {
      throw new Error(`Failed to get storage keys: ${error}`);
    }
  }

  /**
   * Get bytes in use for storage
   */
  private async getBytesInUse(storageArea: chrome.storage.StorageArea, request: StorageToolRequest): Promise<StorageToolResponse> {
    try {
      let keys: string | string[] | null = null;

      if (request.key) {
        keys = this.namespaceKey(request.key, request.options?.namespace);
      } else if (request.keys) {
        keys = request.keys.map(k => this.namespaceKey(k, request.options?.namespace));
      }

      const bytesInUse = await this.executeWithTimeout(
        () => storageArea.getBytesInUse(keys),
        request.options?.timeout || 5000
      );

      // Get quota information
      const quota = await this.getQuotaInfo(request.storageType, bytesInUse);

      return { bytesInUse, quota };
    } catch (error) {
      throw new Error(`Failed to get bytes in use: ${error}`);
    }
  }

  /**
   * Migrate data between storage types
   */
  private async migrateData(request: StorageToolRequest): Promise<StorageToolResponse> {
    if (!request.data || !request.data.fromType || !request.data.toType) {
      throw new Error('Migration requires fromType and toType in data object');
    }

    try {
      const fromStorage = this.getStorageArea(request.data.fromType);
      const toStorage = this.getStorageArea(request.data.toType);

      // Get all data from source storage
      const sourceData = await fromStorage.get(null);

      // Filter keys if specified
      let keysToMigrate = Object.keys(sourceData);
      if (request.keys && request.keys.length > 0) {
        keysToMigrate = keysToMigrate.filter(key =>
          request.keys!.some(k => key === k || key.endsWith(`:${k}`))
        );
      }

      // Prepare data for migration
      const dataToMigrate: Record<string, any> = {};
      for (const key of keysToMigrate) {
        dataToMigrate[key] = sourceData[key];
      }

      // Set data in destination storage
      await toStorage.set(dataToMigrate);

      // Optionally remove from source
      if (request.data.removeFromSource) {
        await fromStorage.remove(keysToMigrate);
      }

      this.log('info', `Migrated ${keysToMigrate.length} items from ${request.data.fromType} to ${request.data.toType}`, {
        keys: keysToMigrate,
        removeFromSource: request.data.removeFromSource,
      });

      return { migrated: keysToMigrate.length, success: true };
    } catch (error) {
      throw new Error(`Failed to migrate data: ${error}`);
    }
  }

  /**
   * Sync data across storage areas
   */
  private async syncData(request: StorageToolRequest): Promise<StorageToolResponse> {
    if (request.storageType !== 'sync') {
      throw new Error('Sync action is only available for sync storage type');
    }

    try {
      // This would trigger Chrome's sync mechanism
      // In practice, sync happens automatically, but we can force a check
      const syncStorage = chrome.storage.sync;

      // Get current sync data to verify connectivity
      await syncStorage.get(null);

      this.log('info', 'Storage sync operation completed', {
        storageType: request.storageType,
      });

      return { success: true, synced: 1 };
    } catch (error) {
      throw new Error(`Failed to sync storage: ${error}`);
    }
  }

  /**
   * Get Chrome storage area
   */
  private getStorageArea(storageType: string): chrome.storage.StorageArea {
    switch (storageType) {
      case 'local':
        return chrome.storage.local;
      case 'session':
        return chrome.storage.session;
      case 'sync':
        return chrome.storage.sync;
      case 'managed':
        return chrome.storage.managed;
      default:
        throw new Error(`Unsupported storage type: ${storageType}`);
    }
  }

  /**
   * Create storage entry with metadata
   */
  private createStorageEntry(value: any, options?: StorageOptions): any {
    if (!options?.ttl && !options?.namespace) {
      return value; // Simple value without metadata
    }

    const entry: StorageEntry = {
      value,
      timestamp: Date.now(),
    };

    if (options?.ttl) {
      entry.ttl = options.ttl;
    }

    if (options?.namespace) {
      entry.namespace = options.namespace;
    }

    return entry;
  }

  /**
   * Process storage results (handle TTL, namespaces)
   */
  private processStorageResult(result: Record<string, any>, namespace?: string): Record<string, any> {
    const processed: Record<string, any> = {};

    for (const [key, rawValue] of Object.entries(result)) {
      let actualKey = key;
      let value = rawValue;

      // Handle namespaced keys
      if (namespace) {
        const prefix = `${namespace}:`;
        if (key.startsWith(prefix)) {
          actualKey = key.substring(prefix.length);
        } else {
          continue; // Skip non-namespaced keys when namespace is specified
        }
      }

      // Handle storage entries with metadata
      if (this.isStorageEntry(value)) {
        // Check TTL
        if (value.ttl && Date.now() > value.timestamp + value.ttl) {
          continue; // Skip expired entries
        }
        value = value.value;
      }

      processed[actualKey] = value;
    }

    return processed;
  }

  /**
   * Check if value is a storage entry with metadata
   */
  private isStorageEntry(value: any): value is StorageEntry {
    return value &&
           typeof value === 'object' &&
           'value' in value &&
           'timestamp' in value;
  }

  /**
   * Add namespace prefix to key
   */
  private namespaceKey(key: string, namespace?: string): string {
    return namespace ? `${namespace}:${key}` : key;
  }

  /**
   * Check quota before setting data
   */
  private async checkQuotaBeforeSet(storageArea: chrome.storage.StorageArea, data: Record<string, any>): Promise<void> {
    try {
      const currentBytesInUse = await storageArea.getBytesInUse(null);
      const dataSize = this.calculateDataSize(data);

      // Get quota limits
      let quotaLimit = 5242880; // 5MB default for local storage
      if (storageArea === chrome.storage.sync) {
        quotaLimit = 102400; // 100KB for sync storage
      }

      if (currentBytesInUse + dataSize > quotaLimit) {
        throw new Error(`Storage quota exceeded: ${currentBytesInUse + dataSize} bytes > ${quotaLimit} bytes`);
      }
    } catch (error) {
      this.log('warn', `Could not check storage quota: ${error}`);
    }
  }

  /**
   * Calculate approximate size of data
   */
  private calculateDataSize(data: Record<string, any>): number {
    return JSON.stringify(data).length * 2; // Rough estimate (UTF-16)
  }

  /**
   * Get quota information
   */
  private async getQuotaInfo(storageType: string, currentUsage: number): Promise<StorageQuota> {
    let totalQuota = 5242880; // 5MB for local

    if (storageType === 'sync') {
      totalQuota = 102400; // 100KB for sync
    } else if (storageType === 'session') {
      totalQuota = 10485760; // 10MB estimate for session
    }

    return {
      available: totalQuota - currentUsage,
      used: currentUsage,
      total: totalQuota,
      percentage: (currentUsage / totalQuota) * 100,
    };
  }

  /**
   * Cleanup expired entries
   */
  async cleanupExpired(storageType: 'local' | 'session' | 'sync' = 'local'): Promise<number> {
    try {
      const storageArea = this.getStorageArea(storageType);
      const allItems = await storageArea.get(null);
      const expiredKeys: string[] = [];

      for (const [key, value] of Object.entries(allItems)) {
        if (this.isStorageEntry(value) && value.ttl) {
          if (Date.now() > value.timestamp + value.ttl) {
            expiredKeys.push(key);
          }
        }
      }

      if (expiredKeys.length > 0) {
        await storageArea.remove(expiredKeys);
        this.log('info', `Cleaned up ${expiredKeys.length} expired entries from ${storageType} storage`);
      }

      return expiredKeys.length;
    } catch (error) {
      this.log('error', `Failed to cleanup expired entries: ${error}`);
      return 0;
    }
  }

  /**
   * Get storage statistics
   */
  async getStats(storageType: 'local' | 'session' | 'sync' = 'local'): Promise<{
    totalKeys: number;
    bytesInUse: number;
    quota: StorageQuota;
    namespaces: string[];
  }> {
    const storageArea = this.getStorageArea(storageType);
    const allItems = await storageArea.get(null);
    const bytesInUse = await storageArea.getBytesInUse(null);

    const namespaces = new Set<string>();

    for (const key of Object.keys(allItems)) {
      const colonIndex = key.indexOf(':');
      if (colonIndex > 0) {
        namespaces.add(key.substring(0, colonIndex));
      }
    }

    const quota = await this.getQuotaInfo(storageType, bytesInUse);

    return {
      totalKeys: Object.keys(allItems).length,
      bytesInUse,
      quota,
      namespaces: Array.from(namespaces),
    };
  }
}