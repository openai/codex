import { describe, it, expect } from 'vitest';
import { checkNodeVersion } from '../../scripts/check_node_version.js';

describe('checkNodeVersion', () => {
  it('accepts Node 22 or newer', () => {
    expect(checkNodeVersion('22.0.0')).toBe(true);
    expect(checkNodeVersion('23.5.1')).toBe(true);
  });

  it('rejects Node versions below 22', () => {
    expect(checkNodeVersion('21.9.0')).toBe(false);
    expect(checkNodeVersion('16.13.0')).toBe(false);
  });
});
