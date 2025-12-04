import { describe, it, expect, beforeAll, beforeEach, afterEach } from 'vitest';
import type { Point, Language, Tree, Edit, TreeCursor } from '../src';
import { Parser } from '../src';
import helper from './helper';

let JavaScript: Language;

interface CursorState {
  nodeType: string;
  nodeIsNamed: boolean;
  startPosition: Point;
  endPosition: Point;
  startIndex: number;
  endIndex: number;
}

describe('Tree', () => {
  let parser: Parser;
  let tree: Tree;

  beforeAll(async () => {
    ({ JavaScript } = await helper);
  });

  beforeEach(() => {
    parser = new Parser();
    parser.setLanguage(JavaScript);
  });

  afterEach(() => {
    parser.delete();
    tree.delete();
  });

  describe('.edit', () => {
    let input: string;
    let edit: Edit;

    it('updates the positions of nodes', () => {
      input = 'abc + cde';
      tree = parser.parse(input)!;
      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (identifier) right: (identifier))))'
      );

      let sumNode = tree.rootNode.firstChild!.firstChild;
      let variableNode1 = sumNode!.firstChild;
      let variableNode2 = sumNode!.lastChild;
      expect(variableNode1!.startIndex).toBe(0);
      expect(variableNode1!.endIndex).toBe(3);
      expect(variableNode2!.startIndex).toBe(6);
      expect(variableNode2!.endIndex).toBe(9);

      [input, edit] = spliceInput(input, input.indexOf('bc'), 0, ' * ');
      expect(input).toBe('a * bc + cde');
      tree.edit(edit);

      sumNode = tree.rootNode.firstChild!.firstChild;
      variableNode1 = sumNode!.firstChild;
      variableNode2 = sumNode!.lastChild;
      expect(variableNode1!.startIndex).toBe(0);
      expect(variableNode1!.endIndex).toBe(6);
      expect(variableNode2!.startIndex).toBe(9);
      expect(variableNode2!.endIndex).toBe(12);

      tree = parser.parse(input, tree)!;
      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier))))'
      );
    });

    it('handles non-ascii characters', () => {
      input = 'αβδ + cde';

      tree = parser.parse(input)!;
      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (identifier) right: (identifier))))'
      );

      let variableNode = tree.rootNode.firstChild!.firstChild!.lastChild;

      [input, edit] = spliceInput(input, input.indexOf('δ'), 0, '👍 * ');
      expect(input).toBe('αβ👍 * δ + cde');
      tree.edit(edit);

      variableNode = tree.rootNode.firstChild!.firstChild!.lastChild;
      expect(variableNode!.startIndex).toBe(input.indexOf('cde'));

      tree = parser.parse(input, tree)!;
      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier))))'
      );
    });
  });

  describe('.getChangedRanges(previous)', () => {
    it('reports the ranges of text whose syntactic meaning has changed', () => {
      let sourceCode = 'abcdefg + hij';
      tree = parser.parse(sourceCode)!;

      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (identifier) right: (identifier))))'
      );

      sourceCode = 'abc + defg + hij';
      tree.edit({
        startIndex: 2,
        oldEndIndex: 2,
        newEndIndex: 5,
        startPosition: { row: 0, column: 2 },
        oldEndPosition: { row: 0, column: 2 },
        newEndPosition: { row: 0, column: 5 },
      });

      const tree2 = parser.parse(sourceCode, tree)!;
      expect(tree2.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (binary_expression left: (identifier) right: (identifier)) right: (identifier))))'
      );

      const ranges = tree.getChangedRanges(tree2);
      expect(ranges).toEqual([
        {
          startIndex: 0,
          endIndex: 'abc + defg'.length,
          startPosition: { row: 0, column: 0 },
          endPosition: { row: 0, column: 'abc + defg'.length },
        },
      ]);

      tree2.delete();
    });

    it('throws an exception if the argument is not a tree', () => {
      tree = parser.parse('abcdefg + hij')!;

      expect(() => {
        tree.getChangedRanges({} as Tree);
      }).toThrow(/Argument must be a Tree/);
    });
  });

  describe('.walk()', () => {
    let cursor: TreeCursor;

    afterEach(() => {
      cursor.delete();
    });

    it('returns a cursor that can be used to walk the tree', () => {
      tree = parser.parse('a * b + c / d')!;
      cursor = tree.walk();

      assertCursorState(cursor, {
        nodeType: 'program',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 13 },
        startIndex: 0,
        endIndex: 13,
      });

      expect(cursor.gotoFirstChild()).toBe(true);
      assertCursorState(cursor, {
        nodeType: 'expression_statement',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 13 },
        startIndex: 0,
        endIndex: 13,
      });

      expect(cursor.gotoFirstChild()).toBe(true);
      assertCursorState(cursor, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 13 },
        startIndex: 0,
        endIndex: 13,
      });

      expect(cursor.gotoFirstChild()).toBe(true);
      assertCursorState(cursor, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 5 },
        startIndex: 0,
        endIndex: 5,
      });

      expect(cursor.gotoFirstChild()).toBe(true);
      expect(cursor.nodeText).toBe('a');
      assertCursorState(cursor, {
        nodeType: 'identifier',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 1 },
        startIndex: 0,
        endIndex: 1,
      });

      expect(cursor.gotoFirstChild()).toBe(false);
      expect(cursor.gotoNextSibling()).toBe(true);
      expect(cursor.nodeText).toBe('*');
      assertCursorState(cursor, {
        nodeType: '*',
        nodeIsNamed: false,
        startPosition: { row: 0, column: 2 },
        endPosition: { row: 0, column: 3 },
        startIndex: 2,
        endIndex: 3,
      });

      expect(cursor.gotoNextSibling()).toBe(true);
      expect(cursor.nodeText).toBe('b');
      assertCursorState(cursor, {
        nodeType: 'identifier',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 4 },
        endPosition: { row: 0, column: 5 },
        startIndex: 4,
        endIndex: 5,
      });

      expect(cursor.gotoNextSibling()).toBe(false);
      expect(cursor.gotoParent()).toBe(true);
      assertCursorState(cursor, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 5 },
        startIndex: 0,
        endIndex: 5,
      });

      expect(cursor.gotoNextSibling()).toBe(true);
      assertCursorState(cursor, {
        nodeType: '+',
        nodeIsNamed: false,
        startPosition: { row: 0, column: 6 },
        endPosition: { row: 0, column: 7 },
        startIndex: 6,
        endIndex: 7,
      });

      expect(cursor.gotoNextSibling()).toBe(true);
      assertCursorState(cursor, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 8 },
        endPosition: { row: 0, column: 13 },
        startIndex: 8,
        endIndex: 13,
      });

      const copy = tree.walk();
      copy.resetTo(cursor);

      expect(copy.gotoPreviousSibling()).toBe(true);
      assertCursorState(copy, {
        nodeType: '+',
        nodeIsNamed: false,
        startPosition: { row: 0, column: 6 },
        endPosition: { row: 0, column: 7 },
        startIndex: 6,
        endIndex: 7,
      });

      expect(copy.gotoPreviousSibling()).toBe(true);
      assertCursorState(copy, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 5 },
        startIndex: 0,
        endIndex: 5,
      });

      expect(copy.gotoLastChild()).toBe(true);
      assertCursorState(copy, {
        nodeType: 'identifier',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 4 },
        endPosition: { row: 0, column: 5 },
        startIndex: 4,
        endIndex: 5,
      });

      expect(copy.gotoParent()).toBe(true);
      expect(copy.gotoParent()).toBe(true);
      expect(copy.nodeType).toBe('binary_expression');
      expect(copy.gotoParent()).toBe(true);
      expect(copy.nodeType).toBe('expression_statement');
      expect(copy.gotoParent()).toBe(true);
      expect(copy.nodeType).toBe('program');
      expect(copy.gotoParent()).toBe(false);
      copy.delete();

      expect(cursor.gotoParent()).toBe(true);
      expect(cursor.nodeType).toBe('binary_expression');
      expect(cursor.gotoParent()).toBe(true);
      expect(cursor.nodeType).toBe('expression_statement');
      expect(cursor.gotoParent()).toBe(true);
      expect(cursor.nodeType).toBe('program');
    });

    it('keeps track of the field name associated with each node', () => {
      tree = parser.parse('a.b();')!;
      cursor = tree.walk();
      cursor.gotoFirstChild();
      cursor.gotoFirstChild();

      expect(cursor.currentNode.type).toBe('call_expression');
      expect(cursor.currentFieldName).toBeNull();

      cursor.gotoFirstChild();
      expect(cursor.currentNode.type).toBe('member_expression');
      expect(cursor.currentFieldName).toBe('function');

      cursor.gotoFirstChild();
      expect(cursor.currentNode.type).toBe('identifier');
      expect(cursor.currentFieldName).toBe('object');

      cursor.gotoNextSibling();
      cursor.gotoNextSibling();
      expect(cursor.currentNode.type).toBe('property_identifier');
      expect(cursor.currentFieldName).toBe('property');

      cursor.gotoParent();
      cursor.gotoNextSibling();
      expect(cursor.currentNode.type).toBe('arguments');
      expect(cursor.currentFieldName).toBe('arguments');
    });

    it('returns a cursor that can be reset anywhere in the tree', () => {
      tree = parser.parse('a * b + c / d')!;
      cursor = tree.walk();
      const root = tree.rootNode.firstChild;

      cursor.reset(root!.firstChild!.firstChild!);
      assertCursorState(cursor, {
        nodeType: 'binary_expression',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 5 },
        startIndex: 0,
        endIndex: 5,
      });

      cursor.gotoFirstChild();
      assertCursorState(cursor, {
        nodeType: 'identifier',
        nodeIsNamed: true,
        startPosition: { row: 0, column: 0 },
        endPosition: { row: 0, column: 1 },
        startIndex: 0,
        endIndex: 1,
      });

      expect(cursor.gotoParent()).toBe(true);
      expect(cursor.gotoParent()).toBe(false);
    });
  });

  describe('.copy', () => {
    let input: string;
    let edit: Edit;

    it('creates another tree that remains stable if the original tree is edited', () => {
      input = 'abc + cde';
      tree = parser.parse(input)!;
      expect(tree.rootNode.toString()).toBe(
        '(program (expression_statement (binary_expression left: (identifier) right: (identifier))))'
      );

      const tree2 = tree.copy();
      [input, edit] = spliceInput(input, 3, 0, '123');
      expect(input).toBe('abc123 + cde');
      tree.edit(edit);

      const leftNode = tree.rootNode.firstChild!.firstChild!.firstChild;
      const leftNode2 = tree2.rootNode.firstChild!.firstChild!.firstChild;
      const rightNode = tree.rootNode.firstChild!.firstChild!.lastChild;
      const rightNode2 = tree2.rootNode.firstChild!.firstChild!.lastChild;
      expect(leftNode!.endIndex).toBe(6);
      expect(leftNode2!.endIndex).toBe(3);
      expect(rightNode!.startIndex).toBe(9);
      expect(rightNode2!.startIndex).toBe(6);

      tree2.delete();
    });
  });
});

function spliceInput(input: string, startIndex: number, lengthRemoved: number, newText: string): [string, Edit] {
  const oldEndIndex = startIndex + lengthRemoved;
  const newEndIndex = startIndex + newText.length;
  const startPosition = getExtent(input.slice(0, startIndex));
  const oldEndPosition = getExtent(input.slice(0, oldEndIndex));
  input = input.slice(0, startIndex) + newText + input.slice(oldEndIndex);
  const newEndPosition = getExtent(input.slice(0, newEndIndex));
  return [
    input,
    {
      startIndex,
      startPosition,
      oldEndIndex,
      oldEndPosition,
      newEndIndex,
      newEndPosition,
    },
  ];
}

// Gets the extent of the text in terms of zero-based row and column numbers.
function getExtent(text: string): Point {
  let row = 0;
  let index = -1;
  let lastIndex = 0;
  while ((index = text.indexOf('\n', index + 1)) !== -1) {
    row++;
    lastIndex = index + 1;
  }
  return { row, column: text.length - lastIndex };
}

function assertCursorState(cursor: TreeCursor, params: CursorState): void {
  expect(cursor.nodeType).toBe(params.nodeType);
  expect(cursor.nodeIsNamed).toBe(params.nodeIsNamed);
  expect(cursor.startPosition).toEqual(params.startPosition);
  expect(cursor.endPosition).toEqual(params.endPosition);
  expect(cursor.startIndex).toEqual(params.startIndex);
  expect(cursor.endIndex).toEqual(params.endIndex);

  const node = cursor.currentNode;
  expect(node.type).toBe(params.nodeType);
  expect(node.isNamed).toBe(params.nodeIsNamed);
  expect(node.startPosition).toEqual(params.startPosition);
  expect(node.endPosition).toEqual(params.endPosition);
  expect(node.startIndex).toEqual(params.startIndex);
  expect(node.endIndex).toEqual(params.endIndex);
}
