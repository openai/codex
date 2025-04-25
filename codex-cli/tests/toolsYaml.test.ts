import { describe, it, expect, beforeAll } from 'vitest';
import fs from 'fs';
import path from 'path';
import yaml from 'js-yaml';

// Valid values
const VALID_PARSERS = ['json', 'json_lines', 'xml', 'plain'];
const VALID_TYPES = ['recon', 'exploit', 'followup'];

describe('tools.yaml structure', () => {
  let toolsObj: any;
  it('sanity', () => {
    expect(true).toBe(true);
  });
  beforeAll(() => {
    const file = path.resolve(__dirname, '../tools.yaml');
    const raw = fs.readFileSync(file, 'utf8');
    toolsObj = yaml.load(raw);
  });

  it('loads and has a tools mapping', () => {
    expect(toolsObj).toBeDefined();
    expect(Object.prototype.hasOwnProperty.call(toolsObj, 'tools')).toBe(true);
    expect(typeof toolsObj.tools).toBe('object');
  });

});