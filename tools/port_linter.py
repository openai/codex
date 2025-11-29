#!/usr/bin/env python
"""
Port Linter - Compare Rust and Kotlin codebases for porting completeness.

Handles naming convention differences:
- Rust: snake_case files (mcp_connection_manager.rs)
- Kotlin: PascalCase files (McpConnectionManager.kt)
- Rust functions: snake_case (create_child_token)
- Kotlin functions: camelCase (createChildToken)
"""

from __future__ import annotations
import math
import os
import re
import sys
from collections import Counter
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional, Iterator
from enum import Enum, auto


# =============================================================================
# Cosine Similarity for Fuzzy Name Matching
# =============================================================================

def normalize_name(name: str) -> str:
    """Normalize a name for comparison: lowercase, split on boundaries."""
    # Convert camelCase/PascalCase to snake_case first
    s1 = re.sub('(.)([A-Z][a-z]+)', r'\1_\2', name)
    s2 = re.sub('([a-z0-9])([A-Z])', r'\1_\2', s1)
    # Lowercase and remove non-alphanumeric
    normalized = s2.lower().replace('-', '_')
    # Remove consecutive underscores
    normalized = re.sub(r'_+', '_', normalized)
    return normalized.strip('_')


def get_name_tokens(name: str) -> list[str]:
    """Split a name into tokens for comparison."""
    normalized = normalize_name(name)
    return [t for t in normalized.split('_') if t]


def get_ngrams(text: str, n: int = 3) -> Counter[str]:
    """Generate character n-grams from text."""
    text = text.lower()
    # Pad for edge n-grams
    padded = f"$${text}$$"
    ngrams: Counter[str] = Counter()
    for i in range(len(padded) - n + 1):
        ngrams[padded[i:i+n]] += 1
    return ngrams


def cosine_similarity(a: Counter[str], b: Counter[str]) -> float:
    """Compute cosine similarity between two n-gram counters."""
    if not a or not b:
        return 0.0

    # Dot product
    dot = sum(a[k] * b[k] for k in a if k in b)

    # Magnitudes
    mag_a = math.sqrt(sum(v * v for v in a.values()))
    mag_b = math.sqrt(sum(v * v for v in b.values()))

    if mag_a == 0 or mag_b == 0:
        return 0.0

    return dot / (mag_a * mag_b)


def jaccard_similarity(a: set[str], b: set[str]) -> float:
    """Compute Jaccard similarity between two sets."""
    if not a or not b:
        return 0.0
    intersection = len(a & b)
    union = len(a | b)
    return intersection / union if union > 0 else 0.0


@dataclass
class NameMatch:
    """A similarity match between names."""
    name: str
    score: float
    match_type: str  # 'exact', 'normalized', 'token', 'ngram'
    location: tuple[Path, int, str]  # (file, line, context)


class SimilarityIndex:
    """Index for fuzzy name matching using multiple similarity metrics."""

    def __init__(self):
        self.names: dict[str, list[tuple[Path, int, str]]] = {}  # name -> [(path, line, context)]
        self.normalized: dict[str, list[str]] = {}  # normalized -> [original names]
        self.ngram_cache: dict[str, Counter[str]] = {}
        self.token_cache: dict[str, set[str]] = {}

    def add(self, name: str, path: Path, line: int, context: str):
        """Add a name to the index."""
        if name not in self.names:
            self.names[name] = []
        self.names[name].append((path, line, context))

        # Index by normalized form
        norm = normalize_name(name)
        if norm not in self.normalized:
            self.normalized[norm] = []
        if name not in self.normalized[norm]:
            self.normalized[norm].append(name)

        # Cache n-grams and tokens
        if name not in self.ngram_cache:
            self.ngram_cache[name] = get_ngrams(norm)
            self.token_cache[name] = set(get_name_tokens(name))

    def find_similar(self, query: str, threshold: float = 0.6, top_k: int = 10) -> list[NameMatch]:
        """Find similar names using multiple similarity metrics."""
        matches: list[NameMatch] = []
        query_norm = normalize_name(query)
        query_ngrams = get_ngrams(query_norm)
        query_tokens = set(get_name_tokens(query))

        for name, locations in self.names.items():
            # Skip exact matches (handled separately)
            if name.lower() == query.lower():
                for loc in locations:
                    matches.append(NameMatch(name, 1.0, 'exact', loc))
                continue

            # Check normalized exact match
            name_norm = normalize_name(name)
            if name_norm == query_norm:
                for loc in locations:
                    matches.append(NameMatch(name, 0.99, 'normalized', loc))
                continue

            # Token-based Jaccard similarity
            name_tokens = self.token_cache.get(name, set(get_name_tokens(name)))
            token_sim = jaccard_similarity(query_tokens, name_tokens)

            # N-gram cosine similarity
            name_ngrams = self.ngram_cache.get(name, get_ngrams(name_norm))
            ngram_sim = cosine_similarity(query_ngrams, name_ngrams)

            # Combined score (weighted average)
            combined = 0.4 * token_sim + 0.6 * ngram_sim

            if combined >= threshold:
                match_type = 'token' if token_sim > ngram_sim else 'ngram'
                for loc in locations:
                    matches.append(NameMatch(name, combined, match_type, loc))

        # Sort by score descending, take top_k
        matches.sort(key=lambda m: -m.score)
        return matches[:top_k]

    def find_all_similar_pairs(self, other: 'SimilarityIndex', threshold: float = 0.7) -> list[tuple[str, str, float, str]]:
        """Find all similar name pairs between this index and another."""
        pairs: list[tuple[str, str, float, str]] = []

        for name in self.names:
            matches = other.find_similar(name, threshold=threshold, top_k=5)
            for match in matches:
                if match.name != name:  # Skip if same name
                    pairs.append((name, match.name, match.score, match.match_type))

        # Deduplicate (A->B and B->A)
        seen = set()
        unique_pairs = []
        for a, b, score, match_type in pairs:
            key = tuple(sorted([a, b]))
            if key not in seen:
                seen.add(key)
                unique_pairs.append((a, b, score, match_type))

        return sorted(unique_pairs, key=lambda x: -x[2])


class NodeType(Enum):
    DIRECTORY = auto()
    FILE = auto()


class MatchStatus(Enum):
    MATCHED = auto()      # Found in both codebases
    RUST_ONLY = auto()    # Only in Rust (missing from Kotlin)
    KOTLIN_ONLY = auto()  # Only in Kotlin (extra/new)
    PARTIAL = auto()      # Directory exists but contents differ


@dataclass
class TreeNode:
    """Node in the codebase tree structure."""
    name: str
    normalized_name: str  # Lowercase, no extension, for comparison
    node_type: NodeType
    path: Path
    children: list[TreeNode] = field(default_factory=list)
    parent: Optional[TreeNode] = field(default=None, repr=False)
    match_status: MatchStatus = MatchStatus.RUST_ONLY
    matched_node: Optional[TreeNode] = field(default=None, repr=False)

    def add_child(self, child: TreeNode) -> TreeNode:
        """Add a child node and set parent reference."""
        child.parent = self
        self.children.append(child)
        return child

    def flatten(self) -> Iterator[TreeNode]:
        """Yield all nodes in the tree (depth-first)."""
        yield self
        for child in self.children:
            yield from child.flatten()

    def flatten_files(self) -> Iterator[TreeNode]:
        """Yield only file nodes."""
        for node in self.flatten():
            if node.node_type == NodeType.FILE:
                yield node

    def flatten_dirs(self) -> Iterator[TreeNode]:
        """Yield only directory nodes."""
        for node in self.flatten():
            if node.node_type == NodeType.DIRECTORY:
                yield node

    def depth(self) -> int:
        """Calculate depth from root."""
        d = 0
        node = self.parent
        while node:
            d += 1
            node = node.parent
        return d

    def relative_path(self, base: Path) -> str:
        """Get path relative to base."""
        try:
            return str(self.path.relative_to(base))
        except ValueError:
            return str(self.path)


class NamingConverter:
    """Convert between Rust and Kotlin naming conventions."""

    @staticmethod
    def snake_to_pascal(name: str) -> str:
        """Convert snake_case to PascalCase.

        mcp_connection_manager -> McpConnectionManager
        thread_history -> ThreadHistory
        """
        parts = name.split('_')
        return ''.join(part.capitalize() for part in parts if part)

    @staticmethod
    def snake_to_camel(name: str) -> str:
        """Convert snake_case to camelCase.

        create_child_token -> createChildToken
        """
        parts = name.split('_')
        if not parts:
            return name
        return parts[0].lower() + ''.join(part.capitalize() for part in parts[1:] if part)

    @staticmethod
    def pascal_to_snake(name: str) -> str:
        """Convert PascalCase to snake_case."""
        # McpConnectionManager -> mcp_connection_manager
        result = re.sub(r'([A-Z])', r'_\1', name).lower()
        return result.lstrip('_')

    @staticmethod
    def camel_to_snake(name: str) -> str:
        """Convert camelCase to snake_case."""
        # createChildToken -> create_child_token
        result = re.sub(r'([A-Z])', r'_\1', name).lower()
        return result.lstrip('_')

    @staticmethod
    def normalize(name: str) -> str:
        """Normalize a name for comparison (lowercase, no separators)."""
        # Remove extension
        name = Path(name).stem if '.' in name else name
        # Remove all separators and lowercase
        return re.sub(r'[_\-]', '', name).lower()


class CodebaseTree:
    """Build and manage a tree representation of a codebase."""

    # File extensions to include
    RUST_EXTENSIONS = {'.rs'}
    KOTLIN_EXTENSIONS = {'.kt', '.kts'}

    # Directories to skip
    SKIP_DIRS = {'target', 'build', '.git', '.gradle', '.idea', 'node_modules'}

    def __init__(self, root_path: Path, language: str):
        self.root_path = root_path
        self.language = language  # 'rust' or 'kotlin'
        self.extensions = self.RUST_EXTENSIONS if language == 'rust' else self.KOTLIN_EXTENSIONS
        self.root: Optional[TreeNode] = None
        self._name_index: dict[str, list[TreeNode]] = {}  # normalized_name -> nodes

    def build(self) -> TreeNode:
        """Build the tree from the filesystem."""
        self.root = self._build_node(self.root_path)
        self._build_index()
        return self.root

    def _build_node(self, path: Path) -> TreeNode:
        """Recursively build tree nodes."""
        name = path.name
        normalized = NamingConverter.normalize(name)

        if path.is_dir():
            node = TreeNode(
                name=name,
                normalized_name=normalized,
                node_type=NodeType.DIRECTORY,
                path=path
            )

            try:
                for child_path in sorted(path.iterdir()):
                    if child_path.name in self.SKIP_DIRS:
                        continue
                    if child_path.is_dir() or child_path.suffix in self.extensions:
                        child_node = self._build_node(child_path)
                        node.add_child(child_node)
            except PermissionError:
                pass

            return node
        else:
            return TreeNode(
                name=name,
                normalized_name=normalized,
                node_type=NodeType.FILE,
                path=path
            )

    def _build_index(self):
        """Build index of normalized names for quick lookup."""
        self._name_index.clear()
        if not self.root:
            return
        for node in self.root.flatten():
            norm = node.normalized_name
            if norm not in self._name_index:
                self._name_index[norm] = []
            self._name_index[norm].append(node)

    def find_by_normalized_name(self, normalized_name: str) -> list[TreeNode]:
        """Find all nodes matching a normalized name."""
        return self._name_index.get(normalized_name, [])

    def file_count(self) -> int:
        """Count total files."""
        if not self.root:
            return 0
        return sum(1 for _ in self.root.flatten_files())

    def dir_count(self) -> int:
        """Count total directories."""
        if not self.root:
            return 0
        return sum(1 for _ in self.root.flatten_dirs())


class PortLinter:
    """Compare Rust and Kotlin codebases for porting completeness."""

    def __init__(self, rust_root: Path, kotlin_root: Path):
        self.rust_root = rust_root
        self.kotlin_root = kotlin_root
        self.rust_tree = CodebaseTree(rust_root, 'rust')
        self.kotlin_tree = CodebaseTree(kotlin_root, 'kotlin')

    def analyze(self):
        """Build trees and perform analysis."""
        print(f"Building Rust tree from: {self.rust_root}")
        self.rust_tree.build()
        print(f"  Found {self.rust_tree.file_count()} files in {self.rust_tree.dir_count()} directories")

        print(f"\nBuilding Kotlin tree from: {self.kotlin_root}")
        self.kotlin_tree.build()
        print(f"  Found {self.kotlin_tree.file_count()} files in {self.kotlin_tree.dir_count()} directories")

        print("\nMatching files...")
        self._match_trees()

    def _match_trees(self):
        """Match nodes between Rust and Kotlin trees."""
        if not self.rust_tree.root or not self.kotlin_tree.root:
            return

        # Match Rust files to Kotlin
        for rust_node in self.rust_tree.root.flatten_files():
            matches = self.kotlin_tree.find_by_normalized_name(rust_node.normalized_name)
            if matches:
                rust_node.match_status = MatchStatus.MATCHED
                rust_node.matched_node = matches[0]  # Take first match
                matches[0].match_status = MatchStatus.MATCHED
                matches[0].matched_node = rust_node
            else:
                rust_node.match_status = MatchStatus.RUST_ONLY

        # Mark Kotlin-only files
        for kotlin_node in self.kotlin_tree.root.flatten_files():
            if kotlin_node.match_status != MatchStatus.MATCHED:
                kotlin_node.match_status = MatchStatus.KOTLIN_ONLY

    def report_missing(self) -> list[TreeNode]:
        """Get Rust files that have no Kotlin equivalent."""
        if not self.rust_tree.root:
            return []
        return [n for n in self.rust_tree.root.flatten_files()
                if n.match_status == MatchStatus.RUST_ONLY]

    def report_matched(self) -> list[tuple[TreeNode, TreeNode]]:
        """Get matched pairs of (Rust, Kotlin) files."""
        if not self.rust_tree.root:
            return []
        pairs = []
        for n in self.rust_tree.root.flatten_files():
            if n.match_status == MatchStatus.MATCHED and n.matched_node:
                pairs.append((n, n.matched_node))
        return pairs

    def report_kotlin_only(self) -> list[TreeNode]:
        """Get Kotlin files that have no Rust equivalent."""
        if not self.kotlin_tree.root:
            return []
        return [n for n in self.kotlin_tree.root.flatten_files()
                if n.match_status == MatchStatus.KOTLIN_ONLY]

    def print_report(self, verbose: bool = False):
        """Print a summary report."""
        missing = self.report_missing()
        matched = self.report_matched()
        kotlin_only = self.report_kotlin_only()

        total_rust = self.rust_tree.file_count()
        total_kotlin = self.kotlin_tree.file_count()

        print("\n" + "=" * 70)
        print("PORT LINTER REPORT")
        print("=" * 70)

        print(f"\nSUMMARY:")
        print(f"  Rust files:      {total_rust}")
        print(f"  Kotlin files:    {total_kotlin}")
        print(f"  Matched:         {len(matched)} ({100*len(matched)/total_rust:.1f}% of Rust)")
        print(f"  Missing (Rust):  {len(missing)} ({100*len(missing)/total_rust:.1f}% of Rust)")
        print(f"  Kotlin-only:     {len(kotlin_only)}")

        if verbose or len(missing) <= 50:
            print(f"\n{'─' * 70}")
            print("MISSING FROM KOTLIN (need to port):")
            print(f"{'─' * 70}")

            # Group by directory
            by_dir: dict[str, list[TreeNode]] = {}
            for node in missing:
                dir_path = str(node.path.parent.relative_to(self.rust_root))
                if dir_path not in by_dir:
                    by_dir[dir_path] = []
                by_dir[dir_path].append(node)

            for dir_path in sorted(by_dir.keys()):
                print(f"\n  {dir_path}/")
                for node in sorted(by_dir[dir_path], key=lambda n: n.name):
                    expected_kt = NamingConverter.snake_to_pascal(node.normalized_name) + ".kt"
                    print(f"    {node.name:40} -> {expected_kt}")

        if verbose and matched:
            print(f"\n{'─' * 70}")
            print("MATCHED FILES:")
            print(f"{'─' * 70}")
            for rust_node, kotlin_node in sorted(matched, key=lambda p: p[0].name):
                rust_rel = rust_node.relative_path(self.rust_root)
                kotlin_rel = kotlin_node.relative_path(self.kotlin_root)
                print(f"  {rust_rel}")
                print(f"    -> {kotlin_rel}")

        if verbose and kotlin_only:
            print(f"\n{'─' * 70}")
            print("KOTLIN-ONLY FILES (no Rust equivalent):")
            print(f"{'─' * 70}")
            for node in sorted(kotlin_only, key=lambda n: str(n.path)):
                print(f"  {node.relative_path(self.kotlin_root)}")

        print("\n" + "=" * 70)


def find_symbol_in_file(file_path: Path, pattern: str) -> list[tuple[int, str]]:
    """Search for a pattern in a file, return (line_number, line) tuples."""
    results = []
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            for i, line in enumerate(f, 1):
                if re.search(pattern, line, re.IGNORECASE):
                    results.append((i, line.rstrip()))
    except Exception:
        pass
    return results


def search_symbol(rust_root: Path, kotlin_root: Path, symbol: str):
    """Search for a symbol in both codebases with convention conversion."""
    print(f"\nSearching for symbol: {symbol}")
    print("=" * 60)

    # Generate all possible name variations
    variations = {
        symbol,
        symbol.lower(),
        NamingConverter.snake_to_pascal(symbol),
        NamingConverter.snake_to_camel(symbol),
        NamingConverter.pascal_to_snake(symbol),
        NamingConverter.camel_to_snake(symbol),
        NamingConverter.normalize(symbol),
    }

    # Remove duplicates while preserving a nice order
    variations = list(dict.fromkeys(variations))

    print(f"Searching variations: {variations}")

    # Build pattern
    pattern = '|'.join(re.escape(v) for v in variations)

    print(f"\nIn Rust ({rust_root}):")
    print("-" * 40)
    rust_count = 0
    for rs_file in rust_root.rglob("*.rs"):
        if any(skip in str(rs_file) for skip in ['target', '.git']):
            continue
        matches = find_symbol_in_file(rs_file, pattern)
        if matches:
            rel_path = rs_file.relative_to(rust_root)
            for line_num, line in matches[:3]:  # Limit to 3 per file
                print(f"  {rel_path}:{line_num}")
                print(f"    {line[:80]}...")
                rust_count += 1
    if rust_count == 0:
        print("  (no matches)")

    print(f"\nIn Kotlin ({kotlin_root}):")
    print("-" * 40)
    kotlin_count = 0
    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue
        matches = find_symbol_in_file(kt_file, pattern)
        if matches:
            rel_path = kt_file.relative_to(kotlin_root)
            for line_num, line in matches[:3]:
                print(f"  {rel_path}:{line_num}")
                print(f"    {line[:80]}...")
                kotlin_count += 1
    if kotlin_count == 0:
        print("  (no matches)")

    print(f"\nTotal: {rust_count} in Rust, {kotlin_count} in Kotlin")


def find_rust_definitions(rust_root: Path) -> dict[str, list[tuple[Path, int, str]]]:
    """Find all type definitions in Rust codebase."""
    # Rust patterns for type definitions
    # Note: pub(crate), pub(super), pub(in path) are all valid visibility modifiers
    patterns = [
        (r'^\s*pub(?:\([^)]+\))?\s+enum\s+(\w+)', 'enum'),
        (r'^\s*pub(?:\([^)]+\))?\s+struct\s+(\w+)', 'struct'),
        (r'^\s*pub(?:\([^)]+\))?\s+trait\s+(\w+)', 'trait'),
        (r'^\s*enum\s+(\w+)', 'enum'),
        (r'^\s*struct\s+(\w+)', 'struct'),
        (r'^\s*trait\s+(\w+)', 'trait'),
    ]

    definitions: dict[str, list[tuple[Path, int, str]]] = {}

    for rs_file in rust_root.rglob("*.rs"):
        if any(skip in str(rs_file) for skip in ['target', '.git']):
            continue

        try:
            with open(rs_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    for pattern, kind in patterns:
                        match = re.match(pattern, line)
                        if match:
                            name = match.group(1)
                            # Normalize: convert to snake_case for comparison
                            normalized = NamingConverter.pascal_to_snake(name).lower()
                            key = f"{normalized}"
                            if key not in definitions:
                                definitions[key] = []
                            definitions[key].append((rs_file, line_num, line.strip()))
        except Exception:
            pass

    return definitions


def print_duplicates_report(kotlin_root: Path, rust_root: Path = None):
    """Find potential duplicate definitions in Kotlin codebase."""
    print("\n" + "=" * 70)
    print("DUPLICATE DETECTION REPORT")
    print("=" * 70)

    # Get Rust definitions if available
    rust_defs: dict[str, list[tuple[Path, int, str]]] = {}
    if rust_root and rust_root.exists():
        print(f"\nScanning Rust codebase: {rust_root}")
        rust_defs = find_rust_definitions(rust_root)
        print(f"  Found {len(rust_defs)} unique type names in Rust")

    # Kotlin patterns for type definitions
    patterns = [
        (r'^\s*(sealed\s+)?class\s+(\w+)', 'class'),
        (r'^\s*enum\s+class\s+(\w+)', 'enum'),
        (r'^\s*data\s+class\s+(\w+)', 'data class'),
        (r'^\s*object\s+(\w+)', 'object'),
        (r'^\s*interface\s+(\w+)', 'interface'),
    ]

    definitions: dict[str, list[tuple[Path, int, str]]] = {}

    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue

        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    for pattern, kind in patterns:
                        match = re.match(pattern, line)
                        if match:
                            # Get the name (last group)
                            name = match.groups()[-1]
                            key = f"{kind}:{name}"
                            if key not in definitions:
                                definitions[key] = []
                            definitions[key].append((kt_file, line_num, line.strip()))
        except Exception:
            pass

    # Find duplicates
    duplicates = {k: v for k, v in definitions.items() if len(v) > 1}

    if duplicates:
        # Categorize duplicates
        porting_mistakes = []  # Kotlin duplicate, Rust has ONE
        inherited_dups = []    # Both Rust and Kotlin have duplicates
        unknown_dups = []      # Can't find in Rust

        for key, locations in duplicates.items():
            kind, name = key.split(':', 1)
            # Convert Kotlin name to snake_case for Rust lookup
            snake_name = NamingConverter.pascal_to_snake(name).lower()
            snake_name_alt = NamingConverter.camel_to_snake(name).lower()

            # Generate transposed variations for matching
            snake_parts = snake_name.split('_')
            names_to_try = [snake_name, snake_name_alt]
            if len(snake_parts) >= 2:
                names_to_try.append('_'.join(reversed(snake_parts)))
                if len(snake_parts) == 2:
                    names_to_try.append(f"{snake_parts[1]}_{snake_parts[0]}")

            rust_matches = []
            for try_name in names_to_try:
                if try_name in rust_defs:
                    rust_matches = rust_defs[try_name]
                    break

            if not rust_matches:
                unknown_dups.append((key, locations, None))
            elif len(rust_matches) == 1:
                porting_mistakes.append((key, locations, rust_matches))
            else:
                inherited_dups.append((key, locations, rust_matches))

        # Print porting mistakes first (these need fixing!)
        if porting_mistakes:
            print(f"\n{'─' * 70}")
            print(f"PORTING MISTAKES ({len(porting_mistakes)}) - Rust has ONE, Kotlin has MULTIPLE:")
            print(f"{'─' * 70}")
            for key, kt_locations, rust_locations in sorted(porting_mistakes):
                kind, name = key.split(':', 1)
                print(f"\n  {kind} {name}:")
                print(f"    Rust (SINGLE definition):")
                for path, line_num, line in rust_locations:
                    rel_path = path.relative_to(rust_root) if rust_root else path
                    print(f"      {rel_path}:{line_num}")
                print(f"    Kotlin (DUPLICATES - need consolidation):")
                for path, line_num, line in kt_locations:
                    rel_path = path.relative_to(kotlin_root)
                    print(f"      {rel_path}:{line_num}")
                    print(f"        {line[:60]}...")

        # Print inherited duplicates (may be intentional)
        if inherited_dups:
            print(f"\n{'─' * 70}")
            print(f"INHERITED FROM RUST ({len(inherited_dups)}) - Both have multiple:")
            print(f"{'─' * 70}")
            for key, kt_locations, rust_locations in sorted(inherited_dups):
                kind, name = key.split(':', 1)
                print(f"\n  {kind} {name}:")
                print(f"    Rust ({len(rust_locations)} definitions)")
                print(f"    Kotlin ({len(kt_locations)} definitions)")

        # Print unknown (not found in Rust) - but search for potential matches
        if unknown_dups:
            print(f"\n{'─' * 70}")
            print(f"KOTLIN-ONLY DUPLICATES ({len(unknown_dups)}) - No exact Rust match:")
            print(f"{'─' * 70}")
            for key, kt_locations, _ in sorted(unknown_dups):
                kind, name = key.split(':', 1)
                print(f"\n  {kind} {name}:")

                # Show Kotlin locations
                print(f"    Kotlin:")
                for path, line_num, line in kt_locations:
                    rel_path = path.relative_to(kotlin_root)
                    print(f"      {rel_path}:{line_num}")
                    print(f"        {line[:60]}...")

                # Search for potential Rust matches (fuzzy)
                snake_name = NamingConverter.pascal_to_snake(name).lower()
                potential_matches = []

                # Try exact match first
                if snake_name in rust_defs:
                    potential_matches.extend(rust_defs[snake_name])

                # Generate transposed variations
                # e.g., "token_usage" -> "usage_token", "FirstLast" -> "LastFirst"
                snake_parts = snake_name.split('_')
                transposed_names = set()
                if len(snake_parts) >= 2:
                    # Reverse all parts
                    transposed_names.add('_'.join(reversed(snake_parts)))
                    # Swap pairs
                    if len(snake_parts) == 2:
                        transposed_names.add(f"{snake_parts[1]}_{snake_parts[0]}")
                    elif len(snake_parts) == 3:
                        # Try different orderings for 3 parts
                        transposed_names.add(f"{snake_parts[2]}_{snake_parts[1]}_{snake_parts[0]}")
                        transposed_names.add(f"{snake_parts[1]}_{snake_parts[0]}_{snake_parts[2]}")
                        transposed_names.add(f"{snake_parts[0]}_{snake_parts[2]}_{snake_parts[1]}")

                # Try partial matches (name contains or is contained)
                for rust_name, rust_locs in rust_defs.items():
                    if rust_name != snake_name:  # Skip exact (already added)
                        # Check if names are related
                        if (snake_name in rust_name or
                            rust_name in snake_name or
                            name.lower() in rust_name or
                            rust_name.replace('_', '') == snake_name.replace('_', '') or
                            rust_name in transposed_names):  # Transposition check
                            potential_matches.extend(rust_locs)

                if potential_matches:
                    print(f"    Potential Rust matches:")
                    seen = set()
                    for path, line_num, line in potential_matches[:5]:  # Limit to 5
                        rel_path = path.relative_to(rust_root) if rust_root else path
                        key = f"{rel_path}:{line_num}"
                        if key not in seen:
                            seen.add(key)
                            print(f"      {rel_path}:{line_num}")
                            print(f"        {line[:60]}...")
                else:
                    print(f"    Potential Rust matches: (none found for '{snake_name}')")

        print(f"\n{'─' * 70}")
        print("SUMMARY - DUPLICATES:")
        print(f"  Porting mistakes (fix these!):  {len(porting_mistakes)}")
        print(f"  Inherited from Rust:            {len(inherited_dups)}")
        print(f"  Kotlin-only (review these):     {len(unknown_dups)}")
        print(f"  Total duplicates:               {len(duplicates)}")

    else:
        print("\nNo obvious duplicates found.")

    # Now find Kotlin-only definitions (not in Rust at all)
    print(f"\n{'=' * 70}")
    print("KOTLIN-ONLY DEFINITIONS (not found in Rust)")
    print("These may be invented code that diverged from the port")
    print(f"{'=' * 70}")

    # All Kotlin definitions (not just duplicates)
    all_kotlin_defs: dict[str, list[tuple[Path, int, str]]] = {}
    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue
        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    for pattern, kind in patterns:
                        match = re.match(pattern, line)
                        if match:
                            name = match.groups()[-1]
                            key = f"{kind}:{name}"
                            if key not in all_kotlin_defs:
                                all_kotlin_defs[key] = []
                            all_kotlin_defs[key].append((kt_file, line_num, line.strip()))
        except Exception:
            pass

    # Find Kotlin definitions with NO Rust equivalent
    kotlin_only_types = []
    for key, locations in all_kotlin_defs.items():
        kind, name = key.split(':', 1)
        snake_name = NamingConverter.pascal_to_snake(name).lower()
        snake_name_alt = NamingConverter.camel_to_snake(name).lower()

        # Generate transposed variations
        snake_parts = snake_name.split('_')
        names_to_try = [snake_name, snake_name_alt]
        if len(snake_parts) >= 2:
            names_to_try.append('_'.join(reversed(snake_parts)))

        # Check if ANY variation exists in Rust
        found_in_rust = False
        for try_name in names_to_try:
            if try_name in rust_defs:
                found_in_rust = True
                break

        if not found_in_rust:
            kotlin_only_types.append((key, locations))

    if kotlin_only_types:
        # Group by kind
        by_kind: dict[str, list] = {}
        for key, locations in kotlin_only_types:
            kind, name = key.split(':', 1)
            if kind not in by_kind:
                by_kind[kind] = []
            by_kind[kind].append((name, locations))

        total_invented = len(kotlin_only_types)
        print(f"\nFound {total_invented} Kotlin types with no Rust equivalent:\n")

        for kind in sorted(by_kind.keys()):
            items = by_kind[kind]
            print(f"  {kind.upper()} ({len(items)}):")
            for name, locations in sorted(items, key=lambda x: x[0]):
                if len(locations) == 1:
                    path, line_num, line = locations[0]
                    rel_path = path.relative_to(kotlin_root)
                    print(f"    {name:40} {rel_path}:{line_num}")
                else:
                    print(f"    {name:40} ({len(locations)} locations)")
            print()

        print(f"{'─' * 70}")
        print("SUMMARY - KOTLIN-ONLY:")
        print(f"  Total Kotlin types not in Rust: {total_invented}")
        for kind in sorted(by_kind.keys()):
            print(f"    {kind}: {len(by_kind[kind])}")
    else:
        print("\nAll Kotlin types have Rust equivalents.")

    print("\n" + "=" * 70)


def check_snake_case_in_kotlin(kotlin_root: Path, rust_defs: dict[str, list] = None):
    """Check for snake_case usage in Kotlin code - this is a convention violation.

    Kotlin should use camelCase for variables/properties and PascalCase for classes.
    Snake_case in Kotlin usually means someone copy-pasted from Rust without converting.
    """
    print("\n" + "=" * 70)
    print("SNAKE_CASE CHECK IN KOTLIN")
    print("Kotlin should use camelCase for properties, not snake_case")
    print("=" * 70)

    # Pattern to find snake_case identifiers in Kotlin code
    # Matches: val foo_bar, var some_thing, fun do_something, parameter names, etc.
    # Excludes: @SerialName("snake_case"), strings, etc.
    snake_case_patterns = [
        # Property/variable declarations with snake_case
        (r'^\s*(?:val|var)\s+([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\s*[=:]', 'property'),
        # Function parameters with snake_case
        (r'(?:^|,|\()\s*(?:val\s+)?([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\s*:', 'parameter'),
        # Property access with snake_case (foo.bar_baz)
        (r'\.([a-z][a-z0-9]*(?:_[a-z0-9]+)+)(?:\s*[=\(\[]|\s*$)', 'access'),
    ]

    # Allowed snake_case names (framework requirements, etc.)
    allowed_snake_case = {
        'is_error', 'call_id',  # May be required for serialization without @SerialName
    }

    violations: list[tuple[Path, int, str, str, str]] = []  # (path, line, identifier, kind, line_content)
    files_checked = 0

    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue
        files_checked += 1

        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    # Skip lines with @SerialName (those are intentional for JSON)
                    if '@SerialName' in line or '@Json' in line:
                        continue
                    # Skip string literals
                    if line.strip().startswith('//') or line.strip().startswith('*'):
                        continue

                    for pattern, kind in snake_case_patterns:
                        for match in re.finditer(pattern, line):
                            identifier = match.group(1)
                            if identifier in allowed_snake_case:
                                continue
                            # Double-check it's actually snake_case
                            if '_' in identifier and not identifier.startswith('_'):
                                violations.append((kt_file, line_num, identifier, kind, line.strip()))
        except Exception:
            pass

    if violations:
        print(f"\nFound {len(violations)} snake_case violations in {files_checked} files:\n")

        # Group by file
        by_file: dict[Path, list] = {}
        for path, line_num, identifier, kind, line_content in violations:
            if path not in by_file:
                by_file[path] = []
            by_file[path].append((line_num, identifier, kind, line_content))

        # Check if identifier exists in Rust (indicates copy-paste without conversion)
        rust_originated = 0
        for path, items in sorted(by_file.items(), key=lambda x: str(x[0])):
            rel_path = path.relative_to(kotlin_root)
            print(f"\n  {rel_path}:")
            for line_num, identifier, kind, line_content in items[:10]:  # Limit per file
                rust_match = ""
                if rust_defs:
                    # Check if this snake_case name exists in Rust
                    if identifier.lower() in rust_defs:
                        rust_match = " ← EXISTS IN RUST (copy-paste?)"
                        rust_originated += 1
                print(f"    {line_num:4}: {identifier:30} ({kind}){rust_match}")
                if len(line_content) <= 80:
                    print(f"          {line_content}")
            if len(items) > 10:
                print(f"    ... and {len(items) - 10} more")

        print(f"\n{'─' * 70}")
        print("SUMMARY - SNAKE_CASE VIOLATIONS:")
        print(f"  Total violations:              {len(violations)}")
        print(f"  Files with violations:         {len(by_file)}")
        if rust_defs:
            print(f"  Matching Rust identifiers:     {rust_originated} (likely copy-paste)")
        print("\nTo fix: Use camelCase (inputTokens, not input_tokens)")
        print("If needed for serialization, use @SerialName(\"snake_case\")")
    else:
        print(f"\nNo snake_case violations found in {files_checked} Kotlin files.")

    print("\n" + "=" * 70)
    return violations


def build_rust_index(rust_root: Path) -> SimilarityIndex:
    """Build a similarity index from Rust type definitions."""
    index = SimilarityIndex()

    patterns = [
        (r'^\s*pub(?:\([^)]+\))?\s+enum\s+(\w+)', 'enum'),
        (r'^\s*pub(?:\([^)]+\))?\s+struct\s+(\w+)', 'struct'),
        (r'^\s*pub(?:\([^)]+\))?\s+trait\s+(\w+)', 'trait'),
        (r'^\s*pub(?:\([^)]+\))?\s+fn\s+(\w+)', 'fn'),
        (r'^\s*enum\s+(\w+)', 'enum'),
        (r'^\s*struct\s+(\w+)', 'struct'),
    ]

    for rs_file in rust_root.rglob("*.rs"):
        if any(skip in str(rs_file) for skip in ['target', '.git']):
            continue
        try:
            with open(rs_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    for pattern, kind in patterns:
                        match = re.match(pattern, line)
                        if match:
                            name = match.group(1)
                            index.add(name, rs_file, line_num, line.strip())
        except Exception:
            pass

    return index


def build_kotlin_index(kotlin_root: Path) -> SimilarityIndex:
    """Build a similarity index from Kotlin type definitions."""
    index = SimilarityIndex()

    patterns = [
        (r'^\s*(sealed\s+)?class\s+(\w+)', 'class'),
        (r'^\s*enum\s+class\s+(\w+)', 'enum'),
        (r'^\s*data\s+class\s+(\w+)', 'data class'),
        (r'^\s*object\s+(\w+)', 'object'),
        (r'^\s*interface\s+(\w+)', 'interface'),
        (r'^\s*fun\s+(\w+)', 'fun'),
    ]

    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue
        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                for line_num, line in enumerate(f, 1):
                    for pattern, kind in patterns:
                        match = re.match(pattern, line)
                        if match:
                            name = match.groups()[-1]
                            index.add(name, kt_file, line_num, line.strip())
        except Exception:
            pass

    return index


def find_similar_names(rust_root: Path, kotlin_root: Path, threshold: float = 0.7, query: str = None):
    """Find similar names between Rust and Kotlin codebases using cosine similarity."""
    print("\n" + "=" * 70)
    print("SIMILARITY MATCHING (Cosine + Jaccard)")
    print(f"Threshold: {threshold:.0%}")
    print("=" * 70)

    print("\nBuilding Rust index...")
    rust_index = build_rust_index(rust_root)
    print(f"  Indexed {len(rust_index.names)} unique Rust names")

    print("Building Kotlin index...")
    kotlin_index = build_kotlin_index(kotlin_root)
    print(f"  Indexed {len(kotlin_index.names)} unique Kotlin names")

    if query:
        # Search for a specific name
        print(f"\n{'─' * 70}")
        print(f"Searching for: '{query}'")
        print(f"{'─' * 70}")

        print("\nIn Rust:")
        rust_matches = rust_index.find_similar(query, threshold=0.5, top_k=10)
        if rust_matches:
            for m in rust_matches:
                rel_path = m.location[0].relative_to(rust_root) if rust_root else m.location[0]
                print(f"  {m.score:.0%} {m.name:40} ({m.match_type})")
                print(f"       {rel_path}:{m.location[1]}")
        else:
            print("  (no matches)")

        print("\nIn Kotlin:")
        kotlin_matches = kotlin_index.find_similar(query, threshold=0.5, top_k=10)
        if kotlin_matches:
            for m in kotlin_matches:
                rel_path = m.location[0].relative_to(kotlin_root) if kotlin_root else m.location[0]
                print(f"  {m.score:.0%} {m.name:40} ({m.match_type})")
                print(f"       {rel_path}:{m.location[1]}")
        else:
            print("  (no matches)")
    else:
        # Find Kotlin names without Rust equivalents
        print(f"\n{'─' * 70}")
        print("KOTLIN NAMES WITHOUT RUST MATCHES")
        print(f"{'─' * 70}")

        kotlin_only: list[tuple[str, Path, int]] = []
        matched: list[tuple[str, str, float]] = []

        for kt_name, locations in kotlin_index.names.items():
            rust_matches = rust_index.find_similar(kt_name, threshold=threshold, top_k=1)
            if rust_matches:
                matched.append((kt_name, rust_matches[0].name, rust_matches[0].score))
            else:
                for loc in locations:
                    kotlin_only.append((kt_name, loc[0], loc[1]))

        # Show matched pairs with their similarity scores
        print(f"\n{'─' * 70}")
        print(f"MATCHED PAIRS ({len(matched)}) - Kotlin ↔ Rust")
        print(f"{'─' * 70}")

        # Group by similarity score
        high_conf = [(k, r, s) for k, r, s in matched if s >= 0.95]
        med_conf = [(k, r, s) for k, r, s in matched if 0.8 <= s < 0.95]
        low_conf = [(k, r, s) for k, r, s in matched if s < 0.8]

        if high_conf:
            print(f"\n  EXACT/NORMALIZED MATCHES ({len(high_conf)}):")
            for kt, rs, score in sorted(high_conf, key=lambda x: x[0])[:20]:
                if kt != rs:
                    print(f"    {kt:40} ↔ {rs:40} ({score:.0%})")

        if med_conf:
            print(f"\n  HIGH CONFIDENCE ({len(med_conf)}):")
            for kt, rs, score in sorted(med_conf, key=lambda x: -x[2])[:30]:
                print(f"    {kt:40} ↔ {rs:40} ({score:.0%})")

        if low_conf:
            print(f"\n  LOWER CONFIDENCE ({len(low_conf)}):")
            for kt, rs, score in sorted(low_conf, key=lambda x: -x[2])[:20]:
                print(f"    {kt:40} ↔ {rs:40} ({score:.0%})")

        # Show unmatched Kotlin names
        if kotlin_only:
            print(f"\n{'─' * 70}")
            print(f"UNMATCHED KOTLIN NAMES ({len(kotlin_only)})")
            print("These may be invented code or use very different naming")
            print(f"{'─' * 70}")

            # Group by first letter for readability
            by_letter: dict[str, list] = {}
            for name, path, line in kotlin_only:
                letter = name[0].upper()
                if letter not in by_letter:
                    by_letter[letter] = []
                by_letter[letter].append((name, path, line))

            for letter in sorted(by_letter.keys()):
                items = by_letter[letter]
                if len(items) <= 10:
                    for name, path, line in items:
                        rel_path = path.relative_to(kotlin_root)
                        print(f"  {name:40} {rel_path}:{line}")
                else:
                    print(f"  [{letter}]: {len(items)} names")

        print(f"\n{'─' * 70}")
        print("SUMMARY:")
        print(f"  Matched pairs:     {len(matched)}")
        print(f"  Unmatched Kotlin:  {len(kotlin_only)}")
        print(f"  Match rate:        {len(matched) / (len(matched) + len(kotlin_only)) * 100:.1f}%")

    print("\n" + "=" * 70)


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Port Linter - Compare Rust and Kotlin codebases",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s --compare                    # Compare file structures
  %(prog)s --compare --focus core       # Compare only core/ directory
  %(prog)s --search session_source      # Search for a symbol
  %(prog)s --compare --verbose          # Detailed comparison report
  %(prog)s --duplicates                 # Find duplicate definitions in Kotlin
  %(prog)s --rust-root /path/to/rust --kotlin-root /path/to/kotlin
        """
    )

    # Paths
    parser.add_argument('--rust-root', type=Path,
                        default=Path(__file__).parent.parent / 'codex-rs',
                        help='Root of Rust codebase')
    parser.add_argument('--kotlin-root', type=Path,
                        default=Path(__file__).parent.parent / 'src',
                        help='Root of Kotlin codebase')

    # Modes
    parser.add_argument('--compare', action='store_true',
                        help='Compare file structures between codebases')
    parser.add_argument('--search', type=str, metavar='SYMBOL',
                        help='Search for a symbol in both codebases')
    parser.add_argument('--duplicates', action='store_true',
                        help='Find duplicate definitions in Kotlin')
    parser.add_argument('--snake-case', action='store_true',
                        help='Check for snake_case violations in Kotlin code')
    parser.add_argument('--similar', action='store_true',
                        help='Find similar names using cosine similarity')
    parser.add_argument('--similar-query', type=str, metavar='NAME',
                        help='Search for similar names to a specific query')
    parser.add_argument('--threshold', type=float, default=0.7,
                        help='Similarity threshold (0.0-1.0, default: 0.7)')
    parser.add_argument('--focus', type=str, metavar='DIR',
                        help='Focus on specific Rust directory (e.g., core, protocol)')
    parser.add_argument('--verbose', '-v', action='store_true',
                        help='Show detailed output')
    parser.add_argument('--matched', action='store_true',
                        help='Show matched files')

    args = parser.parse_args()

    # Validate paths
    if not args.rust_root.exists():
        print(f"Error: Rust root not found: {args.rust_root}", file=sys.stderr)
        sys.exit(1)
    if not args.kotlin_root.exists():
        print(f"Error: Kotlin root not found: {args.kotlin_root}", file=sys.stderr)
        sys.exit(1)

    # Execute requested mode
    if args.similar or args.similar_query:
        find_similar_names(args.rust_root, args.kotlin_root,
                          threshold=args.threshold, query=args.similar_query)
    elif args.snake_case:
        # Get Rust definitions for cross-reference
        rust_defs = find_rust_definitions(args.rust_root) if args.rust_root.exists() else None
        check_snake_case_in_kotlin(args.kotlin_root, rust_defs)
    elif args.duplicates:
        print_duplicates_report(args.kotlin_root, args.rust_root)
    elif args.search:
        search_symbol(args.rust_root, args.kotlin_root, args.search)
    elif args.compare:
        rust_root = args.rust_root
        if args.focus:
            focused = args.rust_root / args.focus
            if focused.exists():
                rust_root = focused
            else:
                print(f"Warning: --focus directory not found: {focused}")

        linter = PortLinter(rust_root, args.kotlin_root)
        linter.analyze()
        linter.print_report(verbose=args.verbose)

        if args.matched:
            matched = linter.report_matched()
            print(f"\n{'─' * 70}")
            print(f"MATCHED FILES ({len(matched)}):")
            print(f"{'─' * 70}")
            for rust_node, kotlin_node in sorted(matched, key=lambda p: p[0].name):
                print(f"  {rust_node.name:40} <-> {kotlin_node.name}")
    else:
        # Default: show comparison
        linter = PortLinter(args.rust_root, args.kotlin_root)
        linter.analyze()
        linter.print_report(verbose=args.verbose)


if __name__ == '__main__':
    main()
