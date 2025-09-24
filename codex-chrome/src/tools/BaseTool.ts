/**
 * Base Tool Class
 *
 * Abstract base class for all browser tools. Provides common functionality
 * including parameter validation, error handling, and execution context.
 */

import {
  ToolDefinition,
  ToolContext,
  ToolExecutionResponse,
  ToolError,
  ParameterProperty,
  ValidationError,
} from './ToolRegistry';

/**
 * Base execution request interface that all tools should extend
 */
export interface BaseToolRequest {
  [key: string]: any;
}

/**
 * Base execution options
 */
export interface BaseToolOptions {
  timeout?: number;
  retries?: number;
  metadata?: Record<string, any>;
}

/**
 * Tool execution result
 */
export interface ToolResult {
  success: boolean;
  data?: any;
  error?: string;
  metadata?: Record<string, any>;
}

/**
 * Abstract base class for all browser tools
 */
export abstract class BaseTool {
  protected abstract toolDefinition: ToolDefinition;

  /**
   * Get the tool definition
   */
  getDefinition(): ToolDefinition {
    return this.toolDefinition;
  }

  /**
   * Execute the tool with the given request
   */
  async execute(request: BaseToolRequest, options?: BaseToolOptions): Promise<ToolResult> {
    const startTime = Date.now();

    try {
      // Validate parameters
      const validationResult = this.validateParameters(request);
      if (!validationResult.valid) {
        return {
          success: false,
          error: `Parameter validation failed: ${validationResult.errors.map(e => e.message).join(', ')}`,
          metadata: {
            validationErrors: validationResult.errors,
            duration: Date.now() - startTime,
          },
        };
      }

      // Apply defaults
      const processedRequest = this.applyDefaults(request);

      // Execute the tool-specific logic
      const result = await this.executeImpl(processedRequest, options);

      return {
        success: true,
        data: result,
        metadata: {
          duration: Date.now() - startTime,
          toolName: this.toolDefinition.name,
          ...options?.metadata,
        },
      };

    } catch (error: any) {
      return {
        success: false,
        error: this.formatError(error),
        metadata: {
          duration: Date.now() - startTime,
          toolName: this.toolDefinition.name,
          errorType: error.constructor.name,
          ...options?.metadata,
        },
      };
    }
  }

  /**
   * Tool-specific implementation - must be implemented by subclasses
   */
  protected abstract executeImpl(
    request: BaseToolRequest,
    options?: BaseToolOptions
  ): Promise<any>;

  /**
   * Validate parameters against the tool's schema
   */
  protected validateParameters(parameters: Record<string, any>): { valid: boolean; errors: ValidationError[] } {
    const errors: ValidationError[] = [];
    const schema = this.toolDefinition.parameters;

    // Check required parameters
    if (schema.required) {
      for (const requiredParam of schema.required) {
        if (!(requiredParam in parameters) || parameters[requiredParam] == null) {
          errors.push({
            parameter: requiredParam,
            message: `Required parameter '${requiredParam}' is missing`,
            code: 'REQUIRED',
          });
        }
      }
    }

    // Validate each parameter
    for (const [paramName, paramValue] of Object.entries(parameters)) {
      const propSchema = schema.properties[paramName];

      if (!propSchema) {
        if (!schema.additionalProperties) {
          errors.push({
            parameter: paramName,
            message: `Unknown parameter '${paramName}'`,
            code: 'UNKNOWN_PARAMETER',
          });
        }
        continue;
      }

      const paramErrors = this.validateParameter(paramName, paramValue, propSchema);
      errors.push(...paramErrors);
    }

    return {
      valid: errors.length === 0,
      errors,
    };
  }

  /**
   * Validate a single parameter against its schema
   */
  protected validateParameter(
    paramName: string,
    value: any,
    schema: ParameterProperty
  ): ValidationError[] {
    const errors: ValidationError[] = [];

    // Handle null/undefined values
    if (value == null) {
      if (schema.default === undefined) {
        errors.push({
          parameter: paramName,
          message: `Parameter '${paramName}' cannot be null or undefined`,
          code: 'NULL_VALUE',
        });
      }
      return errors;
    }

    // Type validation
    const typeError = this.validateType(paramName, value, schema.type);
    if (typeError) {
      errors.push(typeError);
      return errors; // Don't continue if type is wrong
    }

    // Enum validation
    if (schema.enum && !schema.enum.includes(value)) {
      errors.push({
        parameter: paramName,
        message: `Parameter '${paramName}' must be one of: ${schema.enum.join(', ')}`,
        code: 'ENUM_VIOLATION',
      });
    }

    // Array item validation
    if (schema.type === 'array' && schema.items && Array.isArray(value)) {
      for (let i = 0; i < value.length; i++) {
        const itemErrors = this.validateParameter(`${paramName}[${i}]`, value[i], schema.items);
        errors.push(...itemErrors);
      }
    }

    // Object property validation
    if (schema.type === 'object' && schema.properties && typeof value === 'object' && !Array.isArray(value)) {
      for (const [propName, propValue] of Object.entries(value)) {
        const propSchema = schema.properties[propName];
        if (propSchema) {
          const propErrors = this.validateParameter(`${paramName}.${propName}`, propValue, propSchema);
          errors.push(...propErrors);
        }
      }
    }

    return errors;
  }

  /**
   * Validate parameter type
   */
  protected validateType(paramName: string, value: any, expectedType: string): ValidationError | null {
    const actualType = Array.isArray(value) ? 'array' : typeof value;

    switch (expectedType) {
      case 'string':
        if (typeof value !== 'string') {
          return {
            parameter: paramName,
            message: `Parameter '${paramName}' must be a string, got ${actualType}`,
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'number':
        if (typeof value !== 'number' || isNaN(value)) {
          return {
            parameter: paramName,
            message: `Parameter '${paramName}' must be a valid number, got ${actualType}`,
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'boolean':
        if (typeof value !== 'boolean') {
          return {
            parameter: paramName,
            message: `Parameter '${paramName}' must be a boolean, got ${actualType}`,
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'array':
        if (!Array.isArray(value)) {
          return {
            parameter: paramName,
            message: `Parameter '${paramName}' must be an array, got ${actualType}`,
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'object':
        if (typeof value !== 'object' || Array.isArray(value)) {
          return {
            parameter: paramName,
            message: `Parameter '${paramName}' must be an object, got ${actualType}`,
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      default:
        return {
          parameter: paramName,
          message: `Unknown parameter type: ${expectedType}`,
          code: 'UNKNOWN_TYPE',
        };
    }

    return null;
  }

  /**
   * Apply default values to parameters
   */
  protected applyDefaults(parameters: Record<string, any>): Record<string, any> {
    const result = { ...parameters };
    const schema = this.toolDefinition.parameters;

    for (const [propName, propSchema] of Object.entries(schema.properties)) {
      if (propSchema.default !== undefined && !(propName in result)) {
        result[propName] = propSchema.default;
      }
    }

    return result;
  }

  /**
   * Format error message
   */
  protected formatError(error: Error | string): string {
    if (typeof error === 'string') {
      return error;
    }

    if (error instanceof Error) {
      return `${error.name}: ${error.message}`;
    }

    return 'Unknown error occurred';
  }

  /**
   * Create a tool error with consistent structure
   */
  protected createError(code: string, message: string, details?: any): ToolError {
    return {
      code,
      message,
      details,
    };
  }

  /**
   * Validate Chrome extension context
   */
  protected validateChromeContext(): void {
    if (typeof chrome === 'undefined') {
      throw new Error('Chrome extension APIs not available');
    }
  }

  /**
   * Validate that a tab ID is valid
   */
  protected async validateTabId(tabId: number): Promise<chrome.tabs.Tab> {
    this.validateChromeContext();

    try {
      const tab = await chrome.tabs.get(tabId);
      if (!tab) {
        throw new Error(`Tab with ID ${tabId} not found`);
      }
      return tab;
    } catch (error) {
      throw new Error(`Invalid tab ID ${tabId}: ${error}`);
    }
  }

  /**
   * Get active tab
   */
  protected async getActiveTab(): Promise<chrome.tabs.Tab> {
    this.validateChromeContext();

    const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tabs.length === 0) {
      throw new Error('No active tab found');
    }

    return tabs[0];
  }

  /**
   * Execute with retry logic
   */
  protected async executeWithRetry<T>(
    operation: () => Promise<T>,
    maxRetries: number = 3,
    delayMs: number = 1000
  ): Promise<T> {
    let lastError: Error | null = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        return await operation();
      } catch (error) {
        lastError = error instanceof Error ? error : new Error(String(error));

        if (attempt === maxRetries) {
          break;
        }

        // Wait before retrying
        await new Promise(resolve => setTimeout(resolve, delayMs * attempt));
      }
    }

    throw new Error(`Operation failed after ${maxRetries} attempts: ${lastError?.message || 'Unknown error'}`);
  }

  /**
   * Execute with timeout
   */
  protected async executeWithTimeout<T>(
    operation: () => Promise<T>,
    timeoutMs: number = 30000
  ): Promise<T> {
    return Promise.race([
      operation(),
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`Operation timed out after ${timeoutMs}ms`)), timeoutMs)
      ),
    ]);
  }

  /**
   * Log debug information (can be overridden by subclasses)
   */
  protected log(level: 'debug' | 'info' | 'warn' | 'error', message: string, data?: any): void {
    const logData = data ? { data } : {};
    console[level](`[${this.toolDefinition.name}] ${message}`, logData);
  }

  /**
   * Create execution context for the tool
   */
  protected createContext(sessionId: string, turnId: string): ToolContext {
    return {
      sessionId,
      turnId,
      toolName: this.toolDefinition.name,
      metadata: this.toolDefinition.metadata,
    };
  }

  /**
   * Validate required Chrome permissions
   */
  protected async validatePermissions(permissions: string[]): Promise<void> {
    this.validateChromeContext();

    if (chrome.permissions) {
      const hasPermissions = await chrome.permissions.contains({
        permissions,
      });

      if (!hasPermissions) {
        throw new Error(`Missing required permissions: ${permissions.join(', ')}`);
      }
    }
  }

  /**
   * Safe JSON stringify for logging
   */
  protected safeStringify(obj: any, maxDepth: number = 3): string {
    const seen = new WeakSet();

    return JSON.stringify(obj, (key, val) => {
      if (val != null && typeof val === 'object') {
        if (seen.has(val)) {
          return '[Circular]';
        }
        seen.add(val);
      }
      return val;
    }, 2);
  }
}

/**
 * Utility function to create tool definition
 */
export function createToolDefinition(
  name: string,
  description: string,
  properties: Record<string, ParameterProperty>,
  options: {
    required?: string[];
    category?: string;
    version?: string;
    metadata?: Record<string, any>;
  } = {}
): ToolDefinition {
  return {
    name,
    description,
    parameters: {
      type: 'object',
      properties,
      required: options.required || [],
      additionalProperties: false,
    },
    category: options.category,
    version: options.version || '1.0.0',
    metadata: options.metadata,
  };
}