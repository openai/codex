/**
 * Tool Registry for Browser Tools
 *
 * Manages registration, discovery, and execution dispatch for browser tools.
 * Provides a centralized system for tool management with validation and metadata support.
 */

import { Event } from '../protocol/types';
import { EventCollector } from '../tests/utils/test-helpers';

/**
 * Tool definition structure
 */
export interface ToolDefinition {
  name: string;
  description: string;
  parameters: ToolParameterSchema;
  category?: string;
  version?: string;
  metadata?: Record<string, any>;
}

/**
 * JSON Schema for tool parameters
 */
export interface ToolParameterSchema {
  type: 'object';
  properties: Record<string, ParameterProperty>;
  required?: string[];
  additionalProperties?: boolean;
}

/**
 * Parameter property definition
 */
export interface ParameterProperty {
  type: 'string' | 'number' | 'boolean' | 'array' | 'object';
  description?: string;
  enum?: string[];
  items?: ParameterProperty;
  properties?: Record<string, ParameterProperty>;
  default?: any;
}

/**
 * Tool execution request
 */
export interface ToolExecutionRequest {
  toolName: string;
  parameters: Record<string, any>;
  sessionId: string;
  turnId: string;
  timeout?: number;
}

/**
 * Tool execution response
 */
export interface ToolExecutionResponse {
  success: boolean;
  data?: any;
  error?: ToolError;
  duration: number;
  metadata?: Record<string, any>;
}

/**
 * Tool error details
 */
export interface ToolError {
  code: string;
  message: string;
  details?: any;
}

/**
 * Tool discovery query
 */
export interface ToolDiscoveryQuery {
  category?: string;
  namePattern?: string;
  capabilities?: string[];
  version?: string;
}

/**
 * Tool discovery result
 */
export interface ToolDiscoveryResult {
  tools: ToolDefinition[];
  total: number;
  categories: string[];
}

/**
 * Parameter validation result
 */
export interface ToolValidationResult {
  valid: boolean;
  errors: ValidationError[];
}

/**
 * Validation error details
 */
export interface ValidationError {
  parameter: string;
  message: string;
  code: string;
}

/**
 * Tool execution context
 */
export interface ToolContext {
  sessionId: string;
  turnId: string;
  toolName: string;
  metadata?: Record<string, any>;
}

/**
 * Tool handler function signature
 */
export interface ToolHandler {
  (parameters: Record<string, any>, context: ToolContext): Promise<any>;
}

/**
 * Tool registry entry
 */
interface ToolRegistryEntry {
  definition: ToolDefinition;
  handler: ToolHandler;
  registrationTime: number;
}

/**
 * Tool Registry Implementation
 *
 * Provides centralized tool management for the browser tools system.
 * Handles registration, discovery, validation, and execution dispatch.
 */
export class ToolRegistry {
  private tools: Map<string, ToolRegistryEntry> = new Map();
  private eventCollector?: EventCollector;

  constructor(eventCollector?: EventCollector) {
    this.eventCollector = eventCollector;
  }

  /**
   * Register a tool with the registry
   */
  async register(tool: ToolDefinition, handler: ToolHandler): Promise<void> {
    // Validate tool definition
    this.validateToolDefinition(tool);

    // Check for duplicate registration
    if (this.tools.has(tool.name)) {
      throw new Error(`Tool '${tool.name}' is already registered`);
    }

    // Register the tool
    const entry: ToolRegistryEntry = {
      definition: tool,
      handler,
      registrationTime: Date.now(),
    };

    this.tools.set(tool.name, entry);

    // Emit registration event
    this.emitEvent({
      id: `evt_register_${tool.name}`,
      msg: {
        type: 'ToolRegistered',
        data: {
          tool_name: tool.name,
          category: tool.category,
          version: tool.version,
          registration_time: entry.registrationTime,
        },
      },
    });
  }

  /**
   * Unregister a tool from the registry
   */
  async unregister(toolName: string): Promise<void> {
    if (!this.tools.has(toolName)) {
      throw new Error(`Tool '${toolName}' not found`);
    }

    this.tools.delete(toolName);

    // Emit unregistration event
    this.emitEvent({
      id: `evt_unregister_${toolName}`,
      msg: {
        type: 'ToolUnregistered',
        data: {
          tool_name: toolName,
          unregistration_time: Date.now(),
        },
      },
    });
  }

  /**
   * Discover tools based on query criteria
   */
  async discover(query?: ToolDiscoveryQuery): Promise<ToolDiscoveryResult> {
    let tools = Array.from(this.tools.values()).map(entry => entry.definition);

    // Apply filters
    if (query?.category) {
      tools = tools.filter(tool => tool.category === query.category);
    }

    if (query?.namePattern) {
      const regex = new RegExp(query.namePattern, 'i');
      tools = tools.filter(tool => regex.test(tool.name));
    }

    if (query?.capabilities) {
      // Filter by capabilities (metadata-based)
      tools = tools.filter(tool => {
        if (!tool.metadata?.capabilities) return false;
        return query.capabilities!.every(cap =>
          tool.metadata.capabilities.includes(cap)
        );
      });
    }

    if (query?.version) {
      tools = tools.filter(tool => tool.version === query.version);
    }

    // Get all unique categories
    const allTools = Array.from(this.tools.values()).map(entry => entry.definition);
    const categories = [...new Set(allTools.map(tool => tool.category || 'uncategorized'))];

    return {
      tools,
      total: tools.length,
      categories,
    };
  }

  /**
   * Validate tool parameters against schema
   */
  validate(toolName: string, parameters: Record<string, any>): ToolValidationResult {
    const entry = this.tools.get(toolName);
    if (!entry) {
      return {
        valid: false,
        errors: [{
          parameter: '_tool',
          message: `Tool '${toolName}' not found`,
          code: 'NOT_FOUND',
        }],
      };
    }

    const errors: ValidationError[] = [];
    const schema = entry.definition.parameters;

    // Check required parameters
    if (schema.required) {
      for (const requiredParam of schema.required) {
        if (!(requiredParam in parameters) || parameters[requiredParam] == null) {
          errors.push({
            parameter: requiredParam,
            message: 'Required parameter missing',
            code: 'REQUIRED',
          });
        }
      }
    }

    // Validate parameter types and constraints
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

      // Type validation
      const typeError = this.validateParameterType(paramName, paramValue, propSchema);
      if (typeError) {
        errors.push(typeError);
      }

      // Enum validation
      if (propSchema.enum && !propSchema.enum.includes(paramValue)) {
        errors.push({
          parameter: paramName,
          message: `Value must be one of: ${propSchema.enum.join(', ')}`,
          code: 'ENUM_VIOLATION',
        });
      }
    }

    return {
      valid: errors.length === 0,
      errors,
    };
  }

  /**
   * Execute a tool with the given request
   */
  async execute(request: ToolExecutionRequest): Promise<ToolExecutionResponse> {
    const startTime = Date.now();

    try {
      const entry = this.tools.get(request.toolName);
      if (!entry) {
        return {
          success: false,
          error: {
            code: 'TOOL_NOT_FOUND',
            message: `Tool '${request.toolName}' not found`,
          },
          duration: Date.now() - startTime,
        };
      }

      // Validate parameters
      const validationResult = this.validate(request.toolName, request.parameters);
      if (!validationResult.valid) {
        return {
          success: false,
          error: {
            code: 'VALIDATION_ERROR',
            message: 'Parameter validation failed',
            details: validationResult.errors,
          },
          duration: Date.now() - startTime,
        };
      }

      // Emit execution start event
      this.emitEvent({
        id: `evt_exec_start_${request.toolName}`,
        msg: {
          type: 'ToolExecutionStart',
          data: {
            tool_name: request.toolName,
            session_id: request.sessionId,
            turn_id: request.turnId,
            start_time: startTime,
          },
        },
      });

      // Create execution context
      const context: ToolContext = {
        sessionId: request.sessionId,
        turnId: request.turnId,
        toolName: request.toolName,
        metadata: entry.definition.metadata,
      };

      // Execute with timeout
      const timeout = request.timeout || 30000; // 30 second default
      let result: any;

      try {
        result = await Promise.race([
          entry.handler(request.parameters, context),
          new Promise((_, reject) =>
            setTimeout(() => reject(new Error('Tool execution timeout')), timeout)
          ),
        ]);
      } catch (error: any) {
        const isTimeout = error.message.includes('timeout');

        // Emit error/timeout event
        this.emitEvent({
          id: `evt_exec_${isTimeout ? 'timeout' : 'error'}_${request.toolName}`,
          msg: {
            type: isTimeout ? 'ToolExecutionTimeout' : 'ToolExecutionError',
            data: {
              tool_name: request.toolName,
              session_id: request.sessionId,
              error: error.message,
              duration: Date.now() - startTime,
              ...(isTimeout && { timeout_ms: timeout }),
            },
          },
        });

        return {
          success: false,
          error: {
            code: isTimeout ? 'TIMEOUT' : 'EXECUTION_ERROR',
            message: error.message,
            details: error,
          },
          duration: Date.now() - startTime,
        };
      }

      // Emit success event
      this.emitEvent({
        id: `evt_exec_end_${request.toolName}`,
        msg: {
          type: 'ToolExecutionEnd',
          data: {
            tool_name: request.toolName,
            session_id: request.sessionId,
            success: true,
            duration: Date.now() - startTime,
          },
        },
      });

      return {
        success: true,
        data: result,
        duration: Date.now() - startTime,
        metadata: entry.definition.metadata,
      };

    } catch (error: any) {
      // Emit execution error event
      this.emitEvent({
        id: `evt_exec_error_${request.toolName}`,
        msg: {
          type: 'ToolExecutionError',
          data: {
            tool_name: request.toolName,
            session_id: request.sessionId,
            error: error.message,
            duration: Date.now() - startTime,
          },
        },
      });

      return {
        success: false,
        error: {
          code: 'EXECUTION_ERROR',
          message: error.message,
          details: error,
        },
        duration: Date.now() - startTime,
      };
    }
  }

  /**
   * Get tool definition by name
   */
  getTool(name: string): ToolDefinition | null {
    const entry = this.tools.get(name);
    return entry ? entry.definition : null;
  }

  /**
   * List all registered tools
   */
  listTools(): ToolDefinition[] {
    return Array.from(this.tools.values()).map(entry => entry.definition);
  }

  /**
   * Get registry statistics
   */
  getStats() {
    const categories = new Set<string>();
    let totalTools = 0;

    for (const entry of this.tools.values()) {
      totalTools++;
      categories.add(entry.definition.category || 'uncategorized');
    }

    return {
      totalTools,
      categories: Array.from(categories),
      registeredTools: Array.from(this.tools.keys()),
    };
  }

  /**
   * Clear all registered tools
   */
  clear(): void {
    this.tools.clear();
  }

  /**
   * Validate tool definition structure
   */
  private validateToolDefinition(tool: ToolDefinition): void {
    if (!tool.name || typeof tool.name !== 'string' || tool.name.trim() === '') {
      throw new Error('Tool definition missing required field: name');
    }

    if (!tool.description || typeof tool.description !== 'string' || tool.description.trim() === '') {
      throw new Error('Tool definition missing required field: description');
    }

    if (!tool.parameters || typeof tool.parameters !== 'object') {
      throw new Error('Tool definition missing required field: parameters');
    }

    if (tool.parameters.type !== 'object') {
      throw new Error('Tool parameters must be of type "object"');
    }

    if (!tool.parameters.properties || typeof tool.parameters.properties !== 'object') {
      throw new Error('Tool parameters must define properties');
    }
  }

  /**
   * Validate individual parameter type
   */
  private validateParameterType(
    paramName: string,
    value: any,
    schema: ParameterProperty
  ): ValidationError | null {
    const actualType = Array.isArray(value) ? 'array' : typeof value;

    // Handle null/undefined values
    if (value == null) {
      return schema.default !== undefined ? null : {
        parameter: paramName,
        message: 'Parameter value is null or undefined',
        code: 'NULL_VALUE',
      };
    }

    // Type checking
    switch (schema.type) {
      case 'string':
        if (typeof value !== 'string') {
          return {
            parameter: paramName,
            message: 'Expected string type',
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'number':
        if (typeof value !== 'number' || isNaN(value)) {
          return {
            parameter: paramName,
            message: 'Expected number type',
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'boolean':
        if (typeof value !== 'boolean') {
          return {
            parameter: paramName,
            message: 'Expected boolean type',
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      case 'array':
        if (!Array.isArray(value)) {
          return {
            parameter: paramName,
            message: 'Expected array type',
            code: 'TYPE_MISMATCH',
          };
        }
        // Validate array items if schema is provided
        if (schema.items) {
          for (let i = 0; i < value.length; i++) {
            const itemError = this.validateParameterType(`${paramName}[${i}]`, value[i], schema.items);
            if (itemError) {
              return itemError;
            }
          }
        }
        break;

      case 'object':
        if (typeof value !== 'object' || Array.isArray(value)) {
          return {
            parameter: paramName,
            message: 'Expected object type',
            code: 'TYPE_MISMATCH',
          };
        }
        break;

      default:
        return {
          parameter: paramName,
          message: `Unknown type: ${schema.type}`,
          code: 'UNKNOWN_TYPE',
        };
    }

    return null;
  }

  /**
   * Emit event through event collector
   */
  private emitEvent(event: Event): void {
    if (this.eventCollector) {
      this.eventCollector.collect(event);
    }
  }
}

/**
 * Singleton registry instance
 */
export const toolRegistry = new ToolRegistry();