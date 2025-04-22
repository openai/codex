import { describe, test, expect } from 'vitest';
import { calculateContextPercentRemaining } from '../src/utils/model-utils';
import { openAiModelInfo } from '../src/utils/model-info';
import type { ResponseItem } from 'openai/resources/responses/responses.mjs';

describe('Model Utils', () => {
    describe('openAiModelInfo', () => {
        test('model info entries have required properties', () => {
            Object.entries(openAiModelInfo).forEach(([_, info]) => {
                expect(info).toHaveProperty('label');
                expect(info).toHaveProperty('maxContextLength');
                expect(typeof info.label).toBe('string');
                expect(typeof info.maxContextLength).toBe('number');
            });
        });
    });

    describe('calculateContextPercentRemaining', () => {
        test('returns 100% for empty items', () => {
            const result = calculateContextPercentRemaining([], 'gpt-4o');
            expect(result).toBe(100);
        });

        test('calculates percentage correctly for non-empty items', () => {
            const mockItems: Array<ResponseItem> = [
                {
                    id: 'test-id',
                    type: 'message',
                    role: 'user',
                    status: 'completed',
                    content: [
                        {
                            type: 'input_text',
                            text: 'A'.repeat(openAiModelInfo['gpt-4o'].maxContextLength * 0.25 * 4)
                        }
                    ]
                } as ResponseItem
            ];

            const result = calculateContextPercentRemaining(mockItems, 'gpt-4o');
            expect(result).toBeCloseTo(75, 0);
        });

        test('handles unknown models gracefully', () => {
            const mockItems: Array<ResponseItem> = [];

            expect(() => calculateContextPercentRemaining(mockItems, 'unknown-model' as any)).toThrow();
        });
    });
});