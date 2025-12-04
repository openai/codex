import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import helper from './helper';
import type { LookaheadIterator, Language } from '../src';
import { Parser } from '../src';

let JavaScript: Language;
let Rust: Language;

describe('Language', () => {
  beforeAll(async () => ({ JavaScript, Rust } = await helper));

  describe('.name, .version', () => {
    it('returns the name and version of the language', () => {
      expect(JavaScript.name).toBe('javascript');
      expect(JavaScript.abiVersion).toBe(15);
    });
  });

  describe('.fieldIdForName, .fieldNameForId', () => {
    it('converts between the string and integer representations of fields', () => {
      const nameId = JavaScript.fieldIdForName('name');
      const bodyId = JavaScript.fieldIdForName('body');

      expect(nameId).toBeLessThan(JavaScript.fieldCount);
      expect(bodyId).toBeLessThan(JavaScript.fieldCount);
      expect(JavaScript.fieldNameForId(nameId!)).toBe('name');
      expect(JavaScript.fieldNameForId(bodyId!)).toBe('body');
    });

    it('handles invalid inputs', () => {
      expect(JavaScript.fieldIdForName('namezzz')).toBeNull();
      expect(JavaScript.fieldNameForId(-3)).toBeNull();
      expect(JavaScript.fieldNameForId(10000)).toBeNull();
    });
  });

  describe('.idForNodeType, .nodeTypeForId, .nodeTypeIsNamed', () => {
    it('converts between the string and integer representations of a node type', () => {
      const exportStatementId = JavaScript.idForNodeType('export_statement', true)!;
      const starId = JavaScript.idForNodeType('*', false)!;

      expect(exportStatementId).toBeLessThan(JavaScript.nodeTypeCount);
      expect(starId).toBeLessThan(JavaScript.nodeTypeCount);
      expect(JavaScript.nodeTypeIsNamed(exportStatementId)).toBe(true);
      expect(JavaScript.nodeTypeForId(exportStatementId)).toBe('export_statement');
      expect(JavaScript.nodeTypeIsNamed(starId)).toBe(false);
      expect(JavaScript.nodeTypeForId(starId)).toBe('*');
    });

    it('handles invalid inputs', () => {
      expect(JavaScript.nodeTypeForId(-3)).toBeNull();
      expect(JavaScript.nodeTypeForId(10000)).toBeNull();
      expect(JavaScript.idForNodeType('export_statement', false)).toBeNull();
    });
  });

  describe('Supertypes', () => {
    it('gets the supertypes and subtypes of a parser', () => {
      const supertypes = Rust.supertypes;
      const names = supertypes.map((id) => Rust.nodeTypeForId(id));
      expect(names).toEqual([
        '_expression',
        '_literal',
        '_literal_pattern',
        '_pattern',
        '_type'
      ]);

      for (const id of supertypes) {
        const name = Rust.nodeTypeForId(id);
        const subtypes = Rust.subtypes(id);
        let subtypeNames = subtypes.map((id) => Rust.nodeTypeForId(id));
        subtypeNames = [...new Set(subtypeNames)].sort(); // Remove duplicates & sort

        switch (name) {
          case '_literal':
            expect(subtypeNames).toEqual([
              'boolean_literal',
              'char_literal',
              'float_literal',
              'integer_literal',
              'raw_string_literal',
              'string_literal',
            ]);
            break;
          case '_pattern':
            expect(subtypeNames).toEqual([
              '_',
              '_literal_pattern',
              'captured_pattern',
              'const_block',
              'generic_pattern',
              'identifier',
              'macro_invocation',
              'mut_pattern',
              'or_pattern',
              'range_pattern',
              'ref_pattern',
              'reference_pattern',
              'remaining_field_pattern',
              'scoped_identifier',
              'slice_pattern',
              'struct_pattern',
              'tuple_pattern',
              'tuple_struct_pattern',
            ]);
            break;
          case '_type':
            expect(subtypeNames).toEqual([
              'abstract_type',
              'array_type',
              'bounded_type',
              'dynamic_type',
              'function_type',
              'generic_type',
              'macro_invocation',
              'metavariable',
              'never_type',
              'pointer_type',
              'primitive_type',
              'reference_type',
              'removed_trait_bound',
              'scoped_type_identifier',
              'tuple_type',
              'type_identifier',
              'unit_type',
            ]);
            break;
        }
      }
    });
  });
});

describe('Lookahead iterator', () => {
  let lookahead: LookaheadIterator;
  let state: number;

  beforeAll(async () => {
    ({ JavaScript } = await helper);
    const parser = new Parser();
    parser.setLanguage(JavaScript);
    const tree = parser.parse('function fn() {}')!;
    parser.delete();
    const cursor = tree.walk();
    expect(cursor.gotoFirstChild()).toBe(true);
    expect(cursor.gotoFirstChild()).toBe(true);
    state = cursor.currentNode.nextParseState;
    lookahead = JavaScript.lookaheadIterator(state)!;
    expect(lookahead).toBeDefined();
  });

  afterAll(() => { lookahead.delete() });

  const expected = ['(', 'identifier', '*', 'formal_parameters', 'html_comment', 'comment'];

  it('should iterate over valid symbols in the state', () => {
    const symbols = Array.from(lookahead);
    expect(symbols).toEqual(expect.arrayContaining(expected));
    expect(symbols).toHaveLength(expected.length);
  });

  it('should reset to the initial state', () => {
    expect(lookahead.resetState(state)).toBe(true);
    const symbols = Array.from(lookahead);
    expect(symbols).toEqual(expect.arrayContaining(expected));
    expect(symbols).toHaveLength(expected.length);
  });

  it('should reset', () => {
    expect(lookahead.reset(JavaScript, state)).toBe(true);
    const symbols = Array.from(lookahead);
    expect(symbols).toEqual(expect.arrayContaining(expected));
    expect(symbols).toHaveLength(expected.length);
  });
});
