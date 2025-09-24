/**
 * Contract tests for ToolRegistry
 * Tests tool registration, discovery, and execution dispatch
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { EventCollector, createMockToolResult, assertRejects } from '../utils/test-helpers';

// Define ToolRegistry contract interfaces
interface ToolDefinition {
  name: string;
  description: string;
  parameters: ToolParameterSchema;
  category?: string;
  version?: string;
  metadata?: Record<string, any>;
}

interface ToolParameterSchema {
  type: 'object';
  properties: Record<string, ParameterProperty>;
  required?: string[];
  additionalProperties?: boolean;
}

interface ParameterProperty {
  type: 'string' | 'number' | 'boolean' | 'array' | 'object';
  description?: string;
  enum?: string[];
  items?: ParameterProperty;
  properties?: Record<string, ParameterProperty>;
  default?: any;
}

interface ToolExecutionRequest {
  toolName: string;
  parameters: Record<string, any>;
  sessionId: string;
  turnId: string;
  timeout?: number;
}

interface ToolExecutionResponse {
  success: boolean;
  data?: any;
  error?: ToolError;
  duration: number;
  metadata?: Record<string, any>;
}

interface ToolError {
  code: string;
  message: string;
  details?: any;
}

interface ToolDiscoveryQuery {
  category?: string;
  namePattern?: string;
  capabilities?: string[];
  version?: string;
}

interface ToolDiscoveryResult {
  tools: ToolDefinition[];
  total: number;
  categories: string[];
}

interface ToolValidationResult {
  valid: boolean;
  errors: ValidationError[];
}

interface ValidationError {
  parameter: string;
  message: string;
  code: string;
}

interface ToolRegistry {
  register(tool: ToolDefinition, handler: ToolHandler): Promise<void>;
  unregister(toolName: string): Promise<void>;
  discover(query?: ToolDiscoveryQuery): Promise<ToolDiscoveryResult>;
  validate(toolName: string, parameters: Record<string, any>): ToolValidationResult;
  execute(request: ToolExecutionRequest): Promise<ToolExecutionResponse>;
  getTool(name: string): ToolDefinition | null;
  listTools(): ToolDefinition[];
}

interface ToolHandler {
  (parameters: Record<string, any>, context: ToolContext): Promise<any>;
}

interface ToolContext {
  sessionId: string;
  turnId: string;
  toolName: string;
  metadata?: Record<string, any>;
}

describe('ToolRegistry Contract', () => {
  let eventCollector: EventCollector;

  beforeEach(() => {
    eventCollector = new EventCollector();
  });

  describe('Tool Registration', () => {
    it('should register tools with valid definitions', async () => {
      const toolDefinition: ToolDefinition = {
        name: 'test_tool',
        description: 'A test tool for validation',
        parameters: {
          type: 'object',
          properties: {
            message: {
              type: 'string',
              description: 'Message to process',
            },
            count: {
              type: 'number',
              description: 'Number of times to repeat',
              default: 1,
            },
          },
          required: ['message'],
        },
        category: 'testing',
        version: '1.0.0',
      };

      const handler: ToolHandler = async (params, context) => {
        return {
          result: `Processed: ${params.message} (${params.count || 1} times)`,
          context: context.sessionId,
        };
      };

      const mockRegistry: ToolRegistry = {
        async register(tool: ToolDefinition, toolHandler: ToolHandler): Promise<void> {
          // Validate tool definition
          if (!tool.name || !tool.description || !tool.parameters) {
            throw new Error('Invalid tool definition');
          }

          eventCollector.collect({
            id: 'evt_register',
            msg: {
              type: 'ToolRegistered',
              data: {
                tool_name: tool.name,
                category: tool.category,
                version: tool.version,
              },
            },
          });
        },
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(): Promise<ToolExecutionResponse> {
          throw new Error('Not implemented');
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      await mockRegistry.register(toolDefinition, handler);

      const registerEvent = eventCollector.findByType('ToolRegistered');
      expect(registerEvent).toBeDefined();
      expect((registerEvent?.msg as any).data.tool_name).toBe('test_tool');
    });

    it('should reject invalid tool definitions', async () => {
      const invalidTool: Partial<ToolDefinition> = {
        name: '',  // Empty name should be rejected
        description: 'Invalid tool',
      };

      const mockRegistry: ToolRegistry = {
        async register(tool: ToolDefinition): Promise<void> {
          if (!tool.name || !tool.description || !tool.parameters) {
            throw new Error('Tool definition missing required fields');
          }
        },
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(): Promise<ToolExecutionResponse> {
          throw new Error('Not implemented');
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      await assertRejects(
        mockRegistry.register(invalidTool as ToolDefinition, async () => ({})),
        'Tool definition missing required fields'
      );
    });

    it('should support tool unregistration', async () => {
      let registeredTools = new Set(['existing_tool']);

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(toolName: string): Promise<void> {
          if (!registeredTools.has(toolName)) {
            throw new Error(`Tool '${toolName}' not found`);
          }

          registeredTools.delete(toolName);

          eventCollector.collect({
            id: 'evt_unregister',
            msg: {
              type: 'ToolUnregistered',
              data: {
                tool_name: toolName,
              },
            },
          });
        },
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(): Promise<ToolExecutionResponse> {
          throw new Error('Not implemented');
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      await mockRegistry.unregister('existing_tool');

      expect(registeredTools.has('existing_tool')).toBe(false);

      const unregisterEvent = eventCollector.findByType('ToolUnregistered');
      expect(unregisterEvent).toBeDefined();

      // Test unregistering non-existent tool
      await assertRejects(
        mockRegistry.unregister('nonexistent_tool'),
        'Tool \'nonexistent_tool\' not found'
      );
    });
  });

  describe('Tool Discovery', () => {
    it('should support tool discovery with filters', async () => {
      const mockTools: ToolDefinition[] = [
        {
          name: 'browser_tab_create',
          description: 'Create a new browser tab',
          parameters: {
            type: 'object',
            properties: {
              url: { type: 'string', description: 'URL to open' },
            },
            required: ['url'],
          },
          category: 'browser',
          version: '1.0.0',
        },
        {
          name: 'dom_query',
          description: 'Query DOM elements',
          parameters: {
            type: 'object',
            properties: {
              selector: { type: 'string', description: 'CSS selector' },
            },
            required: ['selector'],
          },
          category: 'dom',
          version: '1.0.0',
        },
        {
          name: 'storage_set',
          description: 'Set storage value',
          parameters: {
            type: 'object',
            properties: {
              key: { type: 'string' },
              value: { type: 'string' },
            },
            required: ['key', 'value'],
          },
          category: 'storage',
          version: '1.0.0',
        },
      ];

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(): Promise<void> {},
        async discover(query?: ToolDiscoveryQuery): Promise<ToolDiscoveryResult> {
          let filteredTools = [...mockTools];

          if (query?.category) {
            filteredTools = filteredTools.filter(t => t.category === query.category);
          }

          if (query?.namePattern) {
            const regex = new RegExp(query.namePattern, 'i');
            filteredTools = filteredTools.filter(t => regex.test(t.name));
          }

          const categories = [...new Set(mockTools.map(t => t.category || 'uncategorized'))];

          return {
            tools: filteredTools,
            total: filteredTools.length,
            categories,
          };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(): Promise<ToolExecutionResponse> {
          throw new Error('Not implemented');
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return mockTools;
        },
      };

      // Test discovery without filters
      const allTools = await mockRegistry.discover();
      expect(allTools.tools).toHaveLength(3);
      expect(allTools.categories).toContain('browser');
      expect(allTools.categories).toContain('dom');
      expect(allTools.categories).toContain('storage');

      // Test discovery with category filter
      const browserTools = await mockRegistry.discover({ category: 'browser' });
      expect(browserTools.tools).toHaveLength(1);
      expect(browserTools.tools[0].name).toBe('browser_tab_create');

      // Test discovery with name pattern
      const domTools = await mockRegistry.discover({ namePattern: 'dom_' });
      expect(domTools.tools).toHaveLength(1);
      expect(domTools.tools[0].name).toBe('dom_query');
    });
  });

  describe('Parameter Validation', () => {
    it('should validate tool parameters against schema', () => {
      const toolDef: ToolDefinition = {
        name: 'validation_test',
        description: 'Tool for testing parameter validation',
        parameters: {
          type: 'object',
          properties: {
            requiredString: { type: 'string', description: 'Required string param' },
            optionalNumber: { type: 'number', description: 'Optional number param' },
            enumValue: { type: 'string', enum: ['option1', 'option2', 'option3'] },
          },
          required: ['requiredString'],
        },
      };

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(toolName: string, parameters: Record<string, any>): ToolValidationResult {
          if (toolName !== 'validation_test') {
            return { valid: false, errors: [{ parameter: '_tool', message: 'Tool not found', code: 'NOT_FOUND' }] };
          }

          const errors: ValidationError[] = [];

          // Check required parameters
          if (!parameters.requiredString) {
            errors.push({
              parameter: 'requiredString',
              message: 'Required parameter missing',
              code: 'REQUIRED',
            });
          }

          // Check type validation
          if (parameters.requiredString && typeof parameters.requiredString !== 'string') {
            errors.push({
              parameter: 'requiredString',
              message: 'Expected string type',
              code: 'TYPE_MISMATCH',
            });
          }

          if (parameters.optionalNumber !== undefined && typeof parameters.optionalNumber !== 'number') {
            errors.push({
              parameter: 'optionalNumber',
              message: 'Expected number type',
              code: 'TYPE_MISMATCH',
            });
          }

          // Check enum validation
          if (parameters.enumValue && !['option1', 'option2', 'option3'].includes(parameters.enumValue)) {
            errors.push({
              parameter: 'enumValue',
              message: 'Value must be one of: option1, option2, option3',
              code: 'ENUM_VIOLATION',
            });
          }

          return { valid: errors.length === 0, errors };
        },
        async execute(): Promise<ToolExecutionResponse> {
          throw new Error('Not implemented');
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      // Test valid parameters
      const validParams = { requiredString: 'test', optionalNumber: 42, enumValue: 'option1' };
      const validResult = mockRegistry.validate('validation_test', validParams);
      expect(validResult.valid).toBe(true);
      expect(validResult.errors).toHaveLength(0);

      // Test missing required parameter
      const missingRequired = { optionalNumber: 42 };
      const missingResult = mockRegistry.validate('validation_test', missingRequired);
      expect(missingResult.valid).toBe(false);
      expect(missingResult.errors).toContainEqual({
        parameter: 'requiredString',
        message: 'Required parameter missing',
        code: 'REQUIRED',
      });

      // Test type mismatch
      const typeMismatch = { requiredString: 123, optionalNumber: 'not_a_number' };
      const typeResult = mockRegistry.validate('validation_test', typeMismatch);
      expect(typeResult.valid).toBe(false);
      expect(typeResult.errors).toHaveLength(2);

      // Test enum violation
      const enumViolation = { requiredString: 'test', enumValue: 'invalid_option' };
      const enumResult = mockRegistry.validate('validation_test', enumViolation);
      expect(enumResult.valid).toBe(false);
      expect(enumResult.errors).toContainEqual({
        parameter: 'enumValue',
        message: 'Value must be one of: option1, option2, option3',
        code: 'ENUM_VIOLATION',
      });
    });
  });

  describe('Execution Dispatch', () => {
    it('should dispatch tool execution requests', async () => {
      const mockToolHandler: ToolHandler = async (parameters, context) => {
        eventCollector.collect({
          id: 'evt_exec_start',
          msg: {
            type: 'ToolExecutionStart',
            data: {
              tool_name: context.toolName,
              session_id: context.sessionId,
              turn_id: context.turnId,
            },
          },
        });

        // Simulate processing
        await new Promise(resolve => setTimeout(resolve, 10));

        return {
          result: `Processed ${parameters.message} successfully`,
          timestamp: Date.now(),
        };
      };

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(request: ToolExecutionRequest): Promise<ToolExecutionResponse> {
          const startTime = Date.now();

          try {
            const context: ToolContext = {
              sessionId: request.sessionId,
              turnId: request.turnId,
              toolName: request.toolName,
            };

            const result = await mockToolHandler(request.parameters, context);

            eventCollector.collect({
              id: 'evt_exec_end',
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
            };
          } catch (error: any) {
            eventCollector.collect({
              id: 'evt_exec_error',
              msg: {
                type: 'ToolExecutionError',
                data: {
                  tool_name: request.toolName,
                  error: error.message,
                },
              },
            });

            return {
              success: false,
              error: {
                code: 'EXECUTION_ERROR',
                message: error.message,
              },
              duration: Date.now() - startTime,
            };
          }
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      const request: ToolExecutionRequest = {
        toolName: 'test_tool',
        parameters: { message: 'Hello World' },
        sessionId: 'session_1',
        turnId: 'turn_1',
        timeout: 5000,
      };

      const response = await mockRegistry.execute(request);

      expect(response.success).toBe(true);
      expect(response.data.result).toContain('Hello World');
      expect(response.duration).toBeGreaterThan(0);

      const startEvent = eventCollector.findByType('ToolExecutionStart');
      const endEvent = eventCollector.findByType('ToolExecutionEnd');

      expect(startEvent).toBeDefined();
      expect(endEvent).toBeDefined();
      expect((endEvent?.msg as any).data.success).toBe(true);
    });

    it('should handle tool execution errors', async () => {
      const failingHandler: ToolHandler = async () => {
        throw new Error('Tool execution failed');
      };

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(request: ToolExecutionRequest): Promise<ToolExecutionResponse> {
          const startTime = Date.now();

          try {
            const context: ToolContext = {
              sessionId: request.sessionId,
              turnId: request.turnId,
              toolName: request.toolName,
            };

            await failingHandler(request.parameters, context);

            return {
              success: true,
              data: null,
              duration: Date.now() - startTime,
            };
          } catch (error: any) {
            eventCollector.collect({
              id: 'evt_exec_error',
              msg: {
                type: 'ToolExecutionError',
                data: {
                  tool_name: request.toolName,
                  error: error.message,
                  session_id: request.sessionId,
                },
              },
            });

            return {
              success: false,
              error: {
                code: 'EXECUTION_ERROR',
                message: error.message,
              },
              duration: Date.now() - startTime,
            };
          }
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      const request: ToolExecutionRequest = {
        toolName: 'failing_tool',
        parameters: {},
        sessionId: 'session_1',
        turnId: 'turn_1',
      };

      const response = await mockRegistry.execute(request);

      expect(response.success).toBe(false);
      expect(response.error?.code).toBe('EXECUTION_ERROR');
      expect(response.error?.message).toContain('Tool execution failed');

      const errorEvent = eventCollector.findByType('ToolExecutionError');
      expect(errorEvent).toBeDefined();
    });

    it('should support timeout handling', async () => {
      const slowHandler: ToolHandler = async () => {
        await new Promise(resolve => setTimeout(resolve, 100)); // Simulate slow operation
        return { result: 'Should not reach here' };
      };

      const mockRegistry: ToolRegistry = {
        async register(): Promise<void> {},
        async unregister(): Promise<void> {},
        async discover(): Promise<ToolDiscoveryResult> {
          return { tools: [], total: 0, categories: [] };
        },
        validate(): ToolValidationResult {
          return { valid: true, errors: [] };
        },
        async execute(request: ToolExecutionRequest): Promise<ToolExecutionResponse> {
          const startTime = Date.now();
          const timeout = request.timeout || 5000;

          try {
            const context: ToolContext = {
              sessionId: request.sessionId,
              turnId: request.turnId,
              toolName: request.toolName,
            };

            const result = await Promise.race([
              slowHandler(request.parameters, context),
              new Promise((_, reject) =>
                setTimeout(() => reject(new Error('Tool execution timeout')), timeout)
              ),
            ]);

            return {
              success: true,
              data: result,
              duration: Date.now() - startTime,
            };
          } catch (error: any) {
            const isTimeout = error.message.includes('timeout');

            eventCollector.collect({
              id: 'evt_timeout',
              msg: {
                type: isTimeout ? 'ToolExecutionTimeout' : 'ToolExecutionError',
                data: {
                  tool_name: request.toolName,
                  timeout_ms: timeout,
                },
              },
            });

            return {
              success: false,
              error: {
                code: isTimeout ? 'TIMEOUT' : 'EXECUTION_ERROR',
                message: error.message,
              },
              duration: Date.now() - startTime,
            };
          }
        },
        getTool(): ToolDefinition | null {
          return null;
        },
        listTools(): ToolDefinition[] {
          return [];
        },
      };

      const request: ToolExecutionRequest = {
        toolName: 'slow_tool',
        parameters: {},
        sessionId: 'session_1',
        turnId: 'turn_1',
        timeout: 50, // Short timeout to trigger timeout
      };

      const response = await mockRegistry.execute(request);

      expect(response.success).toBe(false);
      expect(response.error?.code).toBe('TIMEOUT');

      const timeoutEvent = eventCollector.findByType('ToolExecutionTimeout');
      expect(timeoutEvent).toBeDefined();
    });
  });
});