import { describe, it, expect, beforeAll, beforeEach, afterEach } from 'vitest';
import type { Language, Tree, Node } from '../src';
import { Parser } from '../src';
import helper from './helper';

let C: Language;
let JavaScript: Language;
let JSON: Language;
let EmbeddedTemplate: Language;
let Python: Language;

const JSON_EXAMPLE = `
[
  123,
  false,
  {
    "x": null
  }
]
`;

function getAllNodes(tree: Tree): Node[] {
  const result: Node[] = [];
  let visitedChildren = false;
  const cursor = tree.walk();

  while (true) {
    if (!visitedChildren) {
      result.push(cursor.currentNode);
      if (!cursor.gotoFirstChild()) {
        visitedChildren = true;
      }
    } else if (cursor.gotoNextSibling()) {
      visitedChildren = false;
    } else if (!cursor.gotoParent()) {
      break;
    }
  }
  return result;
}

describe('Node', () => {
  let parser: Parser;
  let tree: Tree | null;

  beforeAll(async () => {
    ({ C, EmbeddedTemplate, JavaScript, JSON, Python } = await helper);
  });

  beforeEach(() => {
    tree = null;
    parser = new Parser();
    parser.setLanguage(JavaScript);
  });

  afterEach(() => {
    parser.delete();
    tree!.delete();
  });

  describe('.children', () => {
    it('returns an array of child nodes', () => {
      tree = parser.parse('x10 + 1000')!;
      expect(tree.rootNode.children).toHaveLength(1);
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.children.map(child => child!.type)).toEqual(['identifier', '+', 'number']);
    });
  });

  describe('.namedChildren', () => {
    it('returns an array of named child nodes', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(tree.rootNode.namedChildren).toHaveLength(1);
      expect(sumNode.namedChildren.map(child => child!.type)).toEqual(['identifier', 'number']);
    });
  });

  describe('.childrenForFieldName', () => {
    it('returns an array of child nodes for the given field name', () => {
      parser.setLanguage(Python);
      const source = `
        if one:
            a()
        elif two:
            b()
        elif three:
            c()
        elif four:
    d()`;

      tree = parser.parse(source)!;
      const node = tree.rootNode.firstChild!;
      expect(node.type).toBe('if_statement');
      const alternatives = node.childrenForFieldName('alternative');
      const alternativeTexts = alternatives.map(n => {
        const condition = n!.childForFieldName('condition')!;
        return source.slice(condition.startIndex, condition.endIndex);
      });
      expect(alternativeTexts).toEqual(['two', 'three', 'four']);
    });
  });

  describe('.startIndex and .endIndex', () => {
    it('returns the character index where the node starts/ends in the text', () => {
      tree = parser.parse('a👍👎1 / b👎c👎')!;
      const quotientNode = tree.rootNode.firstChild!.firstChild!;

      expect(quotientNode.startIndex).toBe(0);
      expect(quotientNode.endIndex).toBe(15);
      expect(quotientNode.children.map(child => child!.startIndex)).toEqual([0, 7, 9]);
      expect(quotientNode.children.map(child => child!.endIndex)).toEqual([6, 8, 15]);
    });
  });

  describe('.startPosition and .endPosition', () => {
    it('returns the row and column where the node starts/ends in the text', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.type).toBe('binary_expression');

      expect(sumNode.startPosition).toEqual({ row: 0, column: 0 });
      expect(sumNode.endPosition).toEqual({ row: 0, column: 10 });
      expect(sumNode.children.map((child) => child!.startPosition)).toEqual([
        { row: 0, column: 0 },
        { row: 0, column: 4 },
        { row: 0, column: 6 },
      ]);
      expect(sumNode.children.map((child) => child!.endPosition)).toEqual([
        { row: 0, column: 3 },
        { row: 0, column: 5 },
        { row: 0, column: 10 },
      ]);
    });

    it('handles characters that occupy two UTF16 code units', () => {
      tree = parser.parse('a👍👎1 /\n b👎c👎')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.children.map(child => [child!.startPosition, child!.endPosition])).toEqual([
        [{ row: 0, column: 0 }, { row: 0, column: 6 }],
        [{ row: 0, column: 7 }, { row: 0, column: 8 }],
        [{ row: 1, column: 1 }, { row: 1, column: 7 }]
      ]);
    });
  });

  describe('.parent', () => {
    it('returns the node\'s parent', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!;
      const variableNode = sumNode.firstChild!;
      expect(sumNode.id).not.toBe(variableNode.id);
      expect(sumNode.id).toBe(variableNode.parent!.id);
      expect(tree.rootNode.id).toBe(sumNode.parent!.id);
    });
  });

  describe('.child(), .firstChild, .lastChild', () => {
    it('returns null when the node has no children', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      const variableNode = sumNode.firstChild!;
      expect(variableNode.firstChild).toBeNull();
      expect(variableNode.lastChild).toBeNull();
      expect(variableNode.firstNamedChild).toBeNull();
      expect(variableNode.lastNamedChild).toBeNull();
      expect(variableNode.child(1)).toBeNull();
    });
  });

  describe('.childForFieldName()', () => {
    it('returns node for the given field name', () => {
      tree = parser.parse('class A { b() {} }')!;

      const classNode = tree.rootNode.firstChild!;
      expect(classNode.type).toBe('class_declaration');

      const classNameNode = classNode.childForFieldName('name')!;
      expect(classNameNode.type).toBe('identifier');
      expect(classNameNode.text).toBe('A');

      const bodyNode = classNode.childForFieldName('body')!;
      expect(bodyNode.type).toBe('class_body');
      expect(bodyNode.text).toBe('{ b() {} }');

      const methodNode = bodyNode.firstNamedChild!;
      expect(methodNode.type).toBe('method_definition');
      expect(methodNode.text).toBe('b() {}');
    });
  });

  describe('.childWithDescendant()', () => {
    it('correctly retrieves immediate children', () => {
      const sourceCode = 'let x = 1; console.log(x);';
      tree = parser.parse(sourceCode)!;
      const root = tree.rootNode
      const child = root.children[0].children[0]
      const a = root.childWithDescendant(child)
      expect(a!.startIndex).toBe(0)
      const b = a!.childWithDescendant(child)
      expect(b).toEqual(child)
      const c = b!.childWithDescendant(child)
      expect(c).toBeNull()
    });
  });

  describe('.nextSibling and .previousSibling', () => {
    it('returns the node\'s next and previous sibling', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.children[1]!.id).toBe(sumNode.children[0]!.nextSibling!.id);
      expect(sumNode.children[2]!.id).toBe(sumNode.children[1]!.nextSibling!.id);
      expect(sumNode.children[0]!.id).toBe(sumNode.children[1]!.previousSibling!.id);
      expect(sumNode.children[1]!.id).toBe(sumNode.children[2]!.previousSibling!.id);
    });
  });

  describe('.nextNamedSibling and .previousNamedSibling', () => {
    it('returns the node\'s next and previous named sibling', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.namedChildren[1]!.id).toBe(sumNode.namedChildren[0]!.nextNamedSibling!.id);
      expect(sumNode.namedChildren[0]!.id).toBe(sumNode.namedChildren[1]!.previousNamedSibling!.id);
    });
  });

  describe('.descendantForIndex(min, max)', () => {
    it('returns the smallest node that spans the given range', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;
      expect(sumNode.descendantForIndex(1, 2)!.type).toBe('identifier');
      expect(sumNode.descendantForIndex(4, 4)!.type).toBe('+');

      expect(() => {
        // @ts-expect-error Testing invalid arguments
        sumNode.descendantForIndex(1, {});
      }).toThrow('Arguments must be numbers');

      expect(() => {
        // @ts-expect-error Testing invalid arguments
        sumNode.descendantForIndex(undefined);
      }).toThrow('Arguments must be numbers');
    });
  });

  describe('.namedDescendantForIndex', () => {
    it('returns the smallest named node that spans the given range', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!;
      expect(sumNode.descendantForIndex(1, 2)!.type).toBe('identifier');
      expect(sumNode.descendantForIndex(4, 4)!.type).toBe('+');
    });
  });

  describe('.descendantForPosition', () => {
    it('returns the smallest node that spans the given range', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!;

      expect(sumNode.descendantForPosition({ row: 0, column: 1 }, { row: 0, column: 2 })!.type).toBe('identifier');
      expect(sumNode.descendantForPosition({ row: 0, column: 4 })!.type).toBe('+');

      expect(() => {
        // @ts-expect-error Testing invalid arguments
        sumNode.descendantForPosition(1, {});
      }).toThrow('Arguments must be {row, column} objects');

      expect(() => {
        // @ts-expect-error Testing invalid arguments
        sumNode.descendantForPosition(undefined);
      }).toThrow('Arguments must be {row, column} objects');
    });
  });

  describe('.namedDescendantForPosition(min, max)', () => {
    it('returns the smallest named node that spans the given range', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!;

      expect(sumNode.namedDescendantForPosition({ row: 0, column: 1 }, { row: 0, column: 2 })!.type).toBe('identifier')
      expect(sumNode.namedDescendantForPosition({ row: 0, column: 4 })!.type).toBe('binary_expression');
    });
  });

  describe('.hasError', () => {
    it('returns true if the node contains an error', () => {
      tree = parser.parse('1 + 2 * * 3')!;
      const node = tree.rootNode;
      expect(node.toString()).toBe(
        '(program (expression_statement (binary_expression left: (number) right: (binary_expression left: (number) (ERROR) right: (number)))))'
      );

      const sum = node.firstChild!.firstChild!;
      expect(sum.hasError).toBe(true);
      expect(sum.children[0]!.hasError).toBe(false);
      expect(sum.children[1]!.hasError).toBe(false);
      expect(sum.children[2]!.hasError).toBe(true);
    });
  });

  describe('.isError', () => {
    it('returns true if the node is an error', () => {
      tree = parser.parse('2 * * 3')!;
      const node = tree.rootNode;
      expect(node.toString()).toBe(
        '(program (expression_statement (binary_expression left: (number) (ERROR) right: (number))))'
      );

      const multi = node.firstChild!.firstChild!;
      expect(multi.hasError).toBe(true);
      expect(multi.children[0]!.isError).toBe(false);
      expect(multi.children[1]!.isError).toBe(false);
      expect(multi.children[2]!.isError).toBe(true);
      expect(multi.children[3]!.isError).toBe(false);
    });
  });

  describe('.isMissing', () => {
    it('returns true if the node was inserted via error recovery', () => {
      tree = parser.parse('(2 ||)')!;
      const node = tree.rootNode;
      expect(node.toString()).toBe(
        '(program (expression_statement (parenthesized_expression (binary_expression left: (number) right: (MISSING identifier)))))'
      );

      const sum = node.firstChild!.firstChild!.firstNamedChild!;
      expect(sum.type).toBe('binary_expression');
      expect(sum.hasError).toBe(true);
      expect(sum.children[0]!.isMissing).toBe(false);
      expect(sum.children[1]!.isMissing).toBe(false);
      expect(sum.children[2]!.isMissing).toBe(true);
    });
  });

  describe('.isExtra', () => {
    it('returns true if the node is an extra node like comments', () => {
      tree = parser.parse('foo(/* hi */);')!;
      const node = tree.rootNode;
      const commentNode = node.descendantForIndex(7, 7)!;

      expect(node.type).toBe('program');
      expect(commentNode.type).toBe('comment');
      expect(node.isExtra).toBe(false);
      expect(commentNode.isExtra).toBe(true);
    });
  });

  describe('.text', () => {
    const text = 'α0 / b👎c👎';

    Object.entries({
      '.parse(String)': text,
      '.parse(Function)': (offset: number) => text.slice(offset, offset + 4),
    }).forEach(([method, _parse]) => {
      it(`returns the text of a node generated by ${method}`, () => {
        const [numeratorSrc, denominatorSrc] = text.split(/\s*\/\s+/);
        tree = parser.parse(_parse)!;
        const quotientNode = tree.rootNode.firstChild!.firstChild!;
        const [numerator, slash, denominator] = quotientNode.children;

        expect(tree.rootNode.text).toBe(text);
        expect(denominator!.text).toBe(denominatorSrc);
        expect(quotientNode.text).toBe(text);
        expect(numerator!.text).toBe(numeratorSrc);
        expect(slash!.text).toBe('/');
      });
    });
  });

  describe('.descendantCount', () => {
    it('returns the number of descendants', () => {
      parser.setLanguage(JSON);
      tree = parser.parse(JSON_EXAMPLE)!;
      const valueNode = tree.rootNode;
      const allNodes = getAllNodes(tree);

      expect(valueNode.descendantCount).toBe(allNodes.length);

      const cursor = tree.walk();
      for (let i = 0; i < allNodes.length; i++) {
        const node = allNodes[i];
        cursor.gotoDescendant(i);
        expect(cursor.currentNode.id).toBe(node.id);
      }

      for (let i = allNodes.length - 1; i >= 0; i--) {
        const node = allNodes[i];
        cursor.gotoDescendant(i);
        expect(cursor.currentNode.id).toBe(node.id);
      }
    });

    it('tests a single node tree', () => {
      parser.setLanguage(EmbeddedTemplate);
      tree = parser.parse('hello')!;

      const nodes = getAllNodes(tree);
      expect(nodes).toHaveLength(2);
      expect(tree.rootNode.descendantCount).toBe(2);

      const cursor = tree.walk();

      cursor.gotoDescendant(0);
      expect(cursor.currentDepth).toBe(0);
      expect(cursor.currentNode.id).toBe(nodes[0].id);

      cursor.gotoDescendant(1);
      expect(cursor.currentDepth).toBe(1);
      expect(cursor.currentNode.id).toBe(nodes[1].id);
    });
  });

  describe('.rootNodeWithOffset', () => {
    it('returns the root node of the tree, offset by the given byte offset', () => {
      tree = parser.parse('  if (a) b')!;
      const node = tree.rootNodeWithOffset(6, { row: 2, column: 2 });
      expect(node.startIndex).toBe(8);
      expect(node.endIndex).toBe(16);
      expect(node.startPosition).toEqual({ row: 2, column: 4 });
      expect(node.endPosition).toEqual({ row: 2, column: 12 });

      let child = node.firstChild!.child(2)!;
      expect(child.type).toBe('expression_statement');
      expect(child.startIndex).toBe(15);
      expect(child.endIndex).toBe(16);
      expect(child.startPosition).toEqual({ row: 2, column: 11 });
      expect(child.endPosition).toEqual({ row: 2, column: 12 });

      const cursor = node.walk();
      cursor.gotoFirstChild();
      cursor.gotoFirstChild();
      cursor.gotoNextSibling();
      child = cursor.currentNode;
      expect(child.type).toBe('parenthesized_expression');
      expect(child.startIndex).toBe(11);
      expect(child.endIndex).toBe(14);
      expect(child.startPosition).toEqual({ row: 2, column: 7 });
      expect(child.endPosition).toEqual({ row: 2, column: 10 });
    });
  });

  describe('.parseState, .nextParseState', () => {
    const text = '10 / 5';

    it('returns node parse state ids', () => {
      tree = parser.parse(text)!;
      const quotientNode = tree.rootNode.firstChild!.firstChild!;
      const [numerator, slash, denominator] = quotientNode.children;

      expect(tree.rootNode.parseState).toBe(0);
      // parse states will change on any change to the grammar so test that it
      // returns something instead
      expect(numerator!.parseState).toBeGreaterThan(0);
      expect(slash!.parseState).toBeGreaterThan(0);
      expect(denominator!.parseState).toBeGreaterThan(0);
    });

    it('returns next parse state equal to the language', () => {
      tree = parser.parse(text)!;
      const quotientNode = tree.rootNode.firstChild!.firstChild!;
      quotientNode.children.forEach((node) => {
        expect(node!.nextParseState).toBe(JavaScript.nextState(node!.parseState, node!.grammarId));
      });
    });
  });

  describe('.descendantsOfType("ERROR")', () => {
    it('finds all of the descendants of an ERROR node', () => {
      tree = parser.parse(
        `if ({a: 'b'} {c: 'd'}) {
          // ^ ERROR
          x = function(a) { b; } function(c) { d; }
        }`
      )!;
      const errorNode = tree.rootNode;
      const descendants = errorNode.descendantsOfType('ERROR');
      expect(
        descendants.map((node) => node!.startIndex)
      ).toEqual(
        [4]
      );
    });
  });

  describe('.descendantsOfType', () => {
    it('finds all descendants of a given type in the given range', () => {
      tree = parser.parse('a + 1 * b * 2 + c + 3')!;
      const outerSum = tree.rootNode.firstChild!.firstChild!;

      const descendants = outerSum.descendantsOfType('number', { row: 0, column: 2 }, { row: 0, column: 15 });
      expect(descendants.map(node => node!.startIndex)).toEqual([4, 12]);
      expect(descendants.map(node => node!.endPosition)).toEqual([{ row: 0, column: 5 }, { row: 0, column: 13 }]);
    });
  });



  describe('.firstChildForIndex(index)', () => {
    it('returns the first child that contains or starts after the given index', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;

      expect(sumNode.firstChildForIndex(0)!.type).toBe('identifier');
      expect(sumNode.firstChildForIndex(1)!.type).toBe('identifier');
      expect(sumNode.firstChildForIndex(3)!.type).toBe('+');
      expect(sumNode.firstChildForIndex(5)!.type).toBe('number');
    });
  });

  describe('.firstNamedChildForIndex(index)', () => {
    it('returns the first child that contains or starts after the given index', () => {
      tree = parser.parse('x10 + 1000')!;
      const sumNode = tree.rootNode.firstChild!.firstChild!;

      expect(sumNode.firstNamedChildForIndex(0)!.type).toBe('identifier');
      expect(sumNode.firstNamedChildForIndex(1)!.type).toBe('identifier');
      expect(sumNode.firstNamedChildForIndex(3)!.type).toBe('number');
    });
  });

  describe('.equals(other)', () => {
    it('returns true if the nodes are the same', () => {
      tree = parser.parse('1 + 2')!;

      const sumNode = tree.rootNode.firstChild!.firstChild!;
      const node1 = sumNode.firstChild!;
      const node2 = sumNode.firstChild!;
      expect(node1.equals(node2)).toBe(true);
    });

    it('returns false if the nodes are not the same', () => {
      tree = parser.parse('1 + 2')!;

      const sumNode = tree.rootNode.firstChild!.firstChild!;
      const node1 = sumNode.firstChild!;
      const node2 = node1.nextSibling!;
      expect(node1.equals(node2)).toBe(false);
    });
  });

  describe('.fieldNameForChild(index)', () => {
    it('returns the field of a child or null', () => {
      parser.setLanguage(C);
      tree = parser.parse('int w = x + /* y is special! */ y;')!;

      const translationUnitNode = tree.rootNode;
      const declarationNode = translationUnitNode.firstChild;
      const binaryExpressionNode = declarationNode!
        .childForFieldName('declarator')!
        .childForFieldName('value')!;

      // -------------------
      // left: (identifier)  0
      // operator: "+"       1 <--- (not a named child)
      // (comment)           2 <--- (is an extra)
      // right: (identifier) 3
      // -------------------

      expect(binaryExpressionNode.fieldNameForChild(0)).toBe('left');
      expect(binaryExpressionNode.fieldNameForChild(1)).toBe('operator');
      // The comment should not have a field name, as it's just an extra
      expect(binaryExpressionNode.fieldNameForChild(2)).toBeNull();
      expect(binaryExpressionNode.fieldNameForChild(3)).toBe('right');
      // Negative test - Not a valid child index
      expect(binaryExpressionNode.fieldNameForChild(4)).toBeNull();
    });
  });

  describe('.fieldNameForNamedChild(index)', () => {
    it('returns the field of a named child or null', () => {
      parser.setLanguage(C);
      tree = parser.parse('int w = x + /* y is special! */ y;')!;

      const translationUnitNode = tree.rootNode;
      const declarationNode = translationUnitNode.firstNamedChild;
      const binaryExpressionNode = declarationNode!
        .childForFieldName('declarator')!
        .childForFieldName('value')!;

      // -------------------
      // left: (identifier)  0
      // operator: "+"       _ <--- (not a named child)
      // (comment)           1 <--- (is an extra)
      // right: (identifier) 2
      // -------------------

      expect(binaryExpressionNode.fieldNameForNamedChild(0)).toBe('left');
      // The comment should not have a field name, as it's just an extra
      expect(binaryExpressionNode.fieldNameForNamedChild(1)).toBeNull();
      // The operator is not a named child, so the named child at index 2 is the right child
      expect(binaryExpressionNode.fieldNameForNamedChild(2)).toBe('right');
      // Negative test - Not a valid child index
      expect(binaryExpressionNode.fieldNameForNamedChild(3)).toBeNull();
    });
  });
});
