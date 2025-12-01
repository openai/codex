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
# Port-Lint Comments
# =============================================================================
# Use these comments to control linter behavior:
#
# SUPPRESSION:
#   // port-lint: ignore-duplicate  - Suppress duplicate detection for this type
#   // port-lint: ignore            - General suppression (same as ignore-duplicate)
#
# PROVENANCE (declare Rust source):
#   // port-lint: source core/src/codex.rs
#   // port-lint: source protocol/src/protocol.rs
#
# The source path is relative to the Rust root (codex-rs/).
# This creates an explicit mapping instead of relying on name matching.
#
# Example:
#   // port-lint: source core/src/codex.rs
#   // port-lint: ignore-duplicate - This is a variant type
#   package ai.solace.coder.core.session
#   ...
#   class Codex(...)

PORTLINT_IGNORE_PATTERN = re.compile(r'//\s*port-lint:\s*ignore(?:-duplicate)?', re.IGNORECASE)
PORTLINT_SOURCE_PATTERN = re.compile(r'//\s*port-lint:\s*source\s+(.+)', re.IGNORECASE)


def extract_portlint_source(file_path: Path) -> str | None:
    """Extract the port-lint source annotation from a Kotlin file.

    Returns the Rust source path if found, None otherwise.
    Example: "core/src/codex.rs"
    """
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            # Only check first 50 lines (should be near top of file)
            for i, line in enumerate(f):
                if i > 50:
                    break
                match = PORTLINT_SOURCE_PATTERN.search(line)
                if match:
                    return match.group(1).strip()
    except Exception:
        pass
    return None


def has_portlint_suppression(lines: list[str], line_num: int) -> bool:
    """Check if a line has a port-lint suppression comment.

    Checks:
    1. The line itself (inline comment)
    2. Lines above, scanning back through annotations (@Serializable, @SerialName, etc.)
       until we find a comment, blank line, or other code
    """
    # line_num is 1-indexed, lines is 0-indexed
    idx = line_num - 1

    # Check current line for inline comment
    if idx < len(lines) and PORTLINT_IGNORE_PATTERN.search(lines[idx]):
        return True

    # Scan backwards through annotation lines looking for suppression comment
    # In Kotlin, type definitions can have multiple annotations:
    #   // port-lint: ignore-duplicate
    #   @Serializable
    #   @SerialName("foo")
    #   data class Foo(...)
    scan_idx = idx - 1
    while scan_idx >= 0:
        prev_line = lines[scan_idx].strip()

        # Found a port-lint comment - suppression applies
        if PORTLINT_IGNORE_PATTERN.search(prev_line):
            return True

        # Annotation line - continue scanning backwards
        if prev_line.startswith('@'):
            scan_idx -= 1
            continue

        # Comment line (but not port-lint) - stop scanning
        if prev_line.startswith('//'):
            return False

        # Blank line or other code - stop scanning
        break

    return False


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

    def namespace_path(self, base: Path, language: str) -> str:
        """Get the namespace path for matching across languages.

        For Rust:  codex-rs/core/src/tools/spec.rs -> core.tools.spec
        For Kotlin: nativeMain/kotlin/ai/solace/coder/core/tools/ToolSpec.kt -> core.tools.toolspec

        This allows matching files by their full module path, not just filename.
        """
        try:
            rel = self.path.relative_to(base)
        except ValueError:
            rel = self.path

        parts = list(rel.parts)

        if language == 'rust':
            # Extract crate name and module structure
            # codex-rs/core/src/tools/spec.rs -> ['core', 'src', 'tools', 'spec.rs']
            # We want: core.tools.spec

            # Find 'src' directory - crate name is just before it
            crate_name = None
            if 'src' in parts:
                src_idx = parts.index('src')
                if src_idx > 0:
                    crate_name = parts[src_idx - 1]
                parts = parts[src_idx + 1:]

            # Remove file extension
            if parts:
                parts[-1] = Path(parts[-1]).stem

            # Skip mod.rs and lib.rs (they represent the parent module)
            if parts and parts[-1] in ('mod', 'lib'):
                parts = parts[:-1]

            # Prepend crate name if found
            if crate_name:
                parts = [crate_name] + parts

            # Normalize each part
            normalized = [NamingConverter.normalize(p) for p in parts]

        elif language == 'kotlin':
            # Skip platform dirs and standard package prefix
            # nativeMain/kotlin/ai/solace/coder/core/tools/ToolSpec.kt
            # We want: core.tools.toolspec

            # Find 'kotlin' dir and skip to package root
            if 'kotlin' in parts:
                kotlin_idx = parts.index('kotlin')
                parts = parts[kotlin_idx + 1:]

            # Skip common package prefixes (ai/solace/coder, com/example, etc.)
            # Look for known module roots: core, protocol, exec, mcp, utils, client
            module_roots = {'core', 'protocol', 'exec', 'mcp', 'utils', 'client', 'platform'}
            for i, part in enumerate(parts):
                if part.lower() in module_roots:
                    parts = parts[i:]
                    break

            # Remove file extension
            if parts:
                parts[-1] = Path(parts[-1]).stem

            # Normalize each part
            normalized = [NamingConverter.normalize(p) for p in parts]

        elif language == 'typescript':
            # TypeScript namespace extraction
            # src/components/Button.tsx -> components.button
            # packages/core/src/utils/helpers.ts -> core.utils.helpers

            # Find 'src' directory
            if 'src' in parts:
                src_idx = parts.index('src')
                # Check if there's a package name before src (e.g., packages/core/src)
                package_name = None
                if src_idx > 0 and parts[src_idx - 1] not in ('packages', 'apps', 'libs'):
                    package_name = parts[src_idx - 1]
                elif src_idx > 1 and parts[src_idx - 2] in ('packages', 'apps', 'libs'):
                    package_name = parts[src_idx - 1]
                parts = parts[src_idx + 1:]
                if package_name:
                    parts = [package_name] + parts

            # Remove file extension
            if parts:
                parts[-1] = Path(parts[-1]).stem

            # Skip index files (they represent the parent module)
            if parts and parts[-1] == 'index':
                parts = parts[:-1]

            # Normalize each part
            normalized = [NamingConverter.normalize(p) for p in parts]

        else:
            normalized = [NamingConverter.normalize(p) for p in parts]

        return '.'.join(normalized)


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
    TYPESCRIPT_EXTENSIONS = {'.ts', '.tsx'}

    # Directories to skip
    SKIP_DIRS = {'target', 'build', '.git', '.gradle', '.idea', 'node_modules', 'dist', '.next'}

    def __init__(self, root_path: Path, language: str):
        self.root_path = root_path
        self.language = language  # 'rust', 'kotlin', or 'typescript'
        if language == 'rust':
            self.extensions = self.RUST_EXTENSIONS
        elif language == 'kotlin':
            self.extensions = self.KOTLIN_EXTENSIONS
        elif language == 'typescript':
            self.extensions = self.TYPESCRIPT_EXTENSIONS
        else:
            raise ValueError(f"Unknown language: {language}")
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
        """Build index of normalized names, namespace paths, and source annotations."""
        self._name_index.clear()
        self._namespace_index: dict[str, list[TreeNode]] = {}
        self._source_annotations: dict[Path, str] = {}  # Kotlin file -> Rust source path
        if not self.root:
            return
        for node in self.root.flatten():
            # Index by normalized name
            norm = node.normalized_name
            if norm not in self._name_index:
                self._name_index[norm] = []
            self._name_index[norm].append(node)

            # Index by namespace path (for files only)
            if node.node_type == NodeType.FILE:
                ns_path = node.namespace_path(self.root_path, self.language)
                if ns_path not in self._namespace_index:
                    self._namespace_index[ns_path] = []
                self._namespace_index[ns_path].append(node)

                # Extract source annotations (Kotlin and TypeScript - both use // comments)
                if self.language in ('kotlin', 'typescript'):
                    source = extract_portlint_source(node.path)
                    if source:
                        self._source_annotations[node.path] = source

    def get_source_annotation(self, node: TreeNode) -> str | None:
        """Get the port-lint source annotation for a node, if any."""
        return self._source_annotations.get(node.path)

    def find_by_normalized_name(self, normalized_name: str) -> list[TreeNode]:
        """Find all nodes matching a normalized name."""
        return self._name_index.get(normalized_name, [])

    def find_by_namespace(self, namespace_path: str) -> list[TreeNode]:
        """Find all nodes matching a namespace path."""
        return self._namespace_index.get(namespace_path, [])

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
    """Compare codebases for porting completeness."""

    def __init__(self, source_root: Path, target_root: Path,
                 source_lang: str = 'rust', target_lang: str = 'kotlin'):
        self.source_root = source_root
        self.target_root = target_root
        self.source_lang = source_lang
        self.target_lang = target_lang
        self.source_tree = CodebaseTree(source_root, source_lang)
        self.target_tree = CodebaseTree(target_root, target_lang)

        # Keep legacy aliases for compatibility
        self.rust_root = source_root if source_lang == 'rust' else target_root
        self.kotlin_root = target_root if target_lang == 'kotlin' else None

    def analyze(self):
        """Build trees and perform analysis."""
        print(f"Building {self.source_lang.upper()} tree from: {self.source_root}")
        self.source_tree.build()
        print(f"  Found {self.source_tree.file_count()} files in {self.source_tree.dir_count()} directories")

        print(f"\nBuilding {self.target_lang.upper()} tree from: {self.target_root}")
        self.target_tree.build()
        print(f"  Found {self.target_tree.file_count()} files in {self.target_tree.dir_count()} directories")
        if self.target_tree._source_annotations:
            print(f"  Found {len(self.target_tree._source_annotations)} source annotations")

        print("\nMatching files...")
        self._match_trees()

        # Report match methods
        if hasattr(self, '_annotation_matches') and self._annotation_matches:
            print(f"  {len(self._annotation_matches)} matched via // port-lint: source")
        if hasattr(self, '_namespace_matches') and self._namespace_matches:
            print(f"  {len(self._namespace_matches)} matched via namespace path")
        if hasattr(self, '_name_matches') and self._name_matches:
            print(f"  {len(self._name_matches)} matched via filename only")

    def _match_trees(self):
        """Match nodes between source and target trees.

        Matching priority:
        1. Explicit source annotations (// port-lint: source path/to/file.rs)
        2. Namespace path matching (core.tools.spec)
        3. Filename-only matching (legacy fallback)
        """
        if not self.source_tree.root or not self.target_tree.root:
            return

        # Build reverse index: source relative path -> source node
        source_by_path: dict[str, TreeNode] = {}
        for source_node in self.source_tree.root.flatten_files():
            try:
                rel_path = str(source_node.path.relative_to(self.source_root))
                source_by_path[rel_path] = source_node
                # Also index without leading directories for flexibility
                # e.g., "core/src/codex.rs" also indexed as "src/codex.rs" and "codex.rs"
                parts = rel_path.split('/')
                for i in range(1, len(parts)):
                    partial = '/'.join(parts[i:])
                    if partial not in source_by_path:
                        source_by_path[partial] = source_node
            except ValueError:
                pass

        # Track annotation matches for reporting
        self._annotation_matches: list[tuple[TreeNode, TreeNode, str]] = []  # (target, source, source_path)
        self._namespace_matches: list[tuple[TreeNode, TreeNode]] = []
        self._name_matches: list[tuple[TreeNode, TreeNode]] = []

        # Phase 1: Match target files with explicit source annotations
        for target_node in self.target_tree.root.flatten_files():
            source = self.target_tree.get_source_annotation(target_node)
            if source and source in source_by_path:
                source_node = source_by_path[source]
                target_node.match_status = MatchStatus.MATCHED
                target_node.matched_node = source_node
                source_node.match_status = MatchStatus.MATCHED
                source_node.matched_node = target_node
                self._annotation_matches.append((target_node, source_node, source))

        # Phase 2: Match remaining source files by namespace path
        for source_node in self.source_tree.root.flatten_files():
            if source_node.match_status == MatchStatus.MATCHED:
                continue

            source_ns = source_node.namespace_path(self.source_root, self.source_lang)
            ns_matches = [m for m in self.target_tree.find_by_namespace(source_ns)
                         if m.match_status != MatchStatus.MATCHED]
            if ns_matches:
                source_node.match_status = MatchStatus.MATCHED
                source_node.matched_node = ns_matches[0]
                ns_matches[0].match_status = MatchStatus.MATCHED
                ns_matches[0].matched_node = source_node
                self._namespace_matches.append((ns_matches[0], source_node))
                continue

            # Phase 3: Fallback to filename-only match
            name_matches = self.target_tree.find_by_normalized_name(source_node.normalized_name)
            file_matches = [m for m in name_matches
                           if m.node_type == NodeType.FILE and m.match_status != MatchStatus.MATCHED]
            if file_matches:
                source_node.match_status = MatchStatus.MATCHED
                source_node.matched_node = file_matches[0]
                file_matches[0].match_status = MatchStatus.MATCHED
                file_matches[0].matched_node = source_node
                self._name_matches.append((file_matches[0], source_node))
            else:
                source_node.match_status = MatchStatus.RUST_ONLY  # TODO: rename to SOURCE_ONLY

        # Mark target-only files
        for target_node in self.target_tree.root.flatten_files():
            if target_node.match_status != MatchStatus.MATCHED:
                target_node.match_status = MatchStatus.KOTLIN_ONLY  # TODO: rename to TARGET_ONLY

    def report_missing(self) -> list[TreeNode]:
        """Get source files that have no target equivalent."""
        if not self.source_tree.root:
            return []
        return [n for n in self.source_tree.root.flatten_files()
                if n.match_status == MatchStatus.RUST_ONLY]

    def report_matched(self) -> list[tuple[TreeNode, TreeNode]]:
        """Get matched pairs of (source, target) files."""
        if not self.source_tree.root:
            return []
        pairs = []
        for n in self.source_tree.root.flatten_files():
            if n.match_status == MatchStatus.MATCHED and n.matched_node:
                pairs.append((n, n.matched_node))
        return pairs

    def report_target_only(self) -> list[TreeNode]:
        """Get target files that have no source equivalent."""
        if not self.target_tree.root:
            return []
        return [n for n in self.target_tree.root.flatten_files()
                if n.match_status == MatchStatus.KOTLIN_ONLY]

    # Legacy alias
    def report_kotlin_only(self) -> list[TreeNode]:
        return self.report_target_only()

    def print_report(self, verbose: bool = False):
        """Print a summary report."""
        missing = self.report_missing()
        matched = self.report_matched()
        target_only = self.report_target_only()

        total_source = self.source_tree.file_count()
        total_target = self.target_tree.file_count()

        print("\n" + "=" * 70)
        print("PORT LINTER REPORT")
        print("=" * 70)

        print(f"\nSUMMARY:")
        print(f"  {self.source_lang.capitalize()} files:      {total_source}")
        print(f"  {self.target_lang.capitalize()} files:    {total_target}")
        print(f"  Matched:         {len(matched)} ({100*len(matched)/total_source:.1f}% of {self.source_lang})")
        print(f"  Missing ({self.source_lang}):  {len(missing)} ({100*len(missing)/total_source:.1f}% of {self.source_lang})")
        print(f"  {self.target_lang.capitalize()}-only:     {len(target_only)}")

        if verbose or len(missing) <= 50:
            print(f"\n{'─' * 70}")
            print(f"MISSING FROM {self.target_lang.upper()} (need to port):")
            print(f"{'─' * 70}")

            # Group by directory
            by_dir: dict[str, list[TreeNode]] = {}
            for node in missing:
                dir_path = str(node.path.parent.relative_to(self.source_root))
                if dir_path not in by_dir:
                    by_dir[dir_path] = []
                by_dir[dir_path].append(node)

            for dir_path in sorted(by_dir.keys()):
                print(f"\n  {dir_path}/")
                for node in sorted(by_dir[dir_path], key=lambda n: n.name):
                    # Use original stem (without extension) for target name conversion
                    stem = Path(node.name).stem
                    if self.target_lang == 'kotlin':
                        expected = NamingConverter.snake_to_pascal(stem) + ".kt"
                    elif self.target_lang == 'rust':
                        expected = NamingConverter.pascal_to_snake(stem) + ".rs"
                    else:
                        expected = stem
                    print(f"    {node.name:40} -> {expected}")

        if verbose and matched:
            print(f"\n{'─' * 70}")
            print("MATCHED FILES:")
            print(f"{'─' * 70}")
            for source_node, target_node in sorted(matched, key=lambda p: p[0].name):
                source_rel = source_node.relative_path(self.source_root)
                target_rel = target_node.relative_path(self.target_root)
                print(f"  {source_rel}")
                print(f"    -> {target_rel}")

        if verbose and target_only:
            print(f"\n{'─' * 70}")
            print(f"{self.target_lang.upper()}-ONLY FILES (no {self.source_lang} equivalent):")
            print(f"{'─' * 70}")
            for node in sorted(target_only, key=lambda n: str(n.path)):
                print(f"  {node.relative_path(self.target_root)}")

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


@dataclass
class Definition:
    """A single definition (type, function, property) in source code."""
    name: str
    kind: str  # 'struct', 'enum', 'fn', 'class', 'property', etc.
    line: int
    context: str  # The line of code
    normalized: str  # Normalized name for comparison


def extract_rust_file_definitions(file_path: Path) -> list[Definition]:
    """Extract all definitions from a single Rust file."""
    patterns = [
        (r'^\s*pub(?:\([^)]+\))?\s+enum\s+(\w+)', 'enum'),
        (r'^\s*pub(?:\([^)]+\))?\s+struct\s+(\w+)', 'struct'),
        (r'^\s*pub(?:\([^)]+\))?\s+trait\s+(\w+)', 'trait'),
        (r'^\s*pub(?:\([^)]+\))?\s+fn\s+(\w+)', 'fn'),
        (r'^\s*pub(?:\([^)]+\))?\s+const\s+(\w+)', 'const'),
        (r'^\s*pub(?:\([^)]+\))?\s+type\s+(\w+)', 'type'),
        (r'^\s*enum\s+(\w+)', 'enum'),
        (r'^\s*struct\s+(\w+)', 'struct'),
        (r'^\s*trait\s+(\w+)', 'trait'),
        (r'^\s*fn\s+(\w+)', 'fn'),
        # Struct fields (inside impl or struct)
        (r'^\s*pub\s+(\w+)\s*:', 'field'),
        # Enum variants
        (r'^\s*(\w+)\s*[,\{]', 'variant'),
    ]

    # Common Rust derive traits to skip (not actual definitions to port)
    derive_traits = {
        'Debug', 'Clone', 'Copy', 'Default', 'PartialEq', 'Eq', 'PartialOrd', 'Ord',
        'Hash', 'Serialize', 'Deserialize', 'Display', 'JsonSchema', 'TS', 'EnumIter',
        'Send', 'Sync', 'Sized', 'Unpin', 'From', 'Into', 'TryFrom', 'TryInto',
        'AsRef', 'AsMut', 'Deref', 'DerefMut', 'Drop', 'Error', 'FromStr'
    }

    # Common Rust trait method implementations to skip (handled by Kotlin differently)
    trait_methods = {
        'fmt',          # Display::fmt -> toString()
        'serialize',    # Serialize::serialize -> @Serializable
        'deserialize',  # Deserialize::deserialize -> @Serializable
        'schema_name',  # JsonSchema -> kotlinx.serialization
        'json_schema',  # JsonSchema -> kotlinx.serialization
        'clone',        # Clone::clone -> data class copy()
        'default',      # Default::default -> default values
        'eq', 'ne',     # PartialEq methods -> data class equals()
        'partial_cmp', 'cmp',  # Ord methods -> Comparable
        'hash',         # Hash::hash -> data class hashCode()
        'as_ref', 'as_mut',    # AsRef/AsMut
        'deref', 'deref_mut',  # Deref/DerefMut
        'drop',         # Drop::drop -> close()/finalize
        'from', 'into', 'try_from', 'try_into',  # Conversion traits
        'from_str',     # FromStr::from_str
        'formatter',    # Internal formatter factory (ICU specific)
    }

    # Patterns for Rust-specific helper functions/types to skip
    rust_helper_patterns = [
        r'^should_serialize_',   # serde skip_serializing_if helpers
        r'^is_default$',         # serde default check helpers
        r'^is_empty$',           # serde skip_serializing_if helpers
        r'_placeholder$',        # placeholder functions
        r'Serde$',               # Internal serde helper types (e.g., FunctionCallOutputPayloadSerde)
        r'^make_.*_formatter$',  # Locale formatter factory functions
        r'^convert_',            # Internal conversion functions
        r'_with_formatter$',     # Internal formatter helper functions
    ]

    definitions: list[Definition] = []
    in_derive_block = False
    in_test_module = False  # Track if we're in a #[cfg(test)] module

    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            for line_num, line in enumerate(f, 1):
                stripped = line.strip()

                # Track if we're inside a #[derive(...)] block
                if stripped.startswith('#[derive('):
                    in_derive_block = True
                if in_derive_block:
                    if ')]' in stripped:
                        in_derive_block = False
                    continue  # Skip lines inside derive blocks

                # Track if we're in a test module
                if '#[cfg(test)]' in stripped:
                    in_test_module = True
                    continue

                # Skip test functions (prefixed with test_ or inside #[test])
                if in_test_module or stripped.startswith('#[test]') or stripped.startswith('#[tokio::test]'):
                    continue

                # Skip attribute lines
                if stripped.startswith('#['):
                    continue

                for pattern, kind in patterns:
                    match = re.match(pattern, line)
                    if match:
                        name = match.group(1)
                        # Skip common false positives, derive traits, and trait method implementations
                        if name in ('self', 'Self', 'super', 'crate', 'pub', 'let', 'mut', 'if', 'else', 'match', 'for', 'while', 'loop', 'return', 'break', 'continue', 'f', 'e', 'v', 'x', 'y', 'i', 'n', 's', 'ok', 'err', 'Ok', 'Err', 'Some', 'None'):
                            continue
                        if name in derive_traits:
                            continue
                        if name in trait_methods:
                            continue
                        # Skip Rust-specific helper patterns (use search for non-anchored patterns)
                        if any(re.search(pat, name) for pat in rust_helper_patterns):
                            continue
                        normalized = NamingConverter.normalize(name)
                        definitions.append(Definition(
                            name=name,
                            kind=kind,
                            line=line_num,
                            context=line.strip(),
                            normalized=normalized
                        ))
                        break  # Only match one pattern per line
    except Exception:
        pass
    return definitions


def extract_kotlin_file_definitions(file_path: Path) -> list[Definition]:
    """Extract all definitions from a single Kotlin file."""
    patterns = [
        (r'^\s*sealed\s+class\s+(\w+)', 'sealed class'),
        (r'^\s*data\s+class\s+(\w+)', 'data class'),
        (r'^\s*enum\s+class\s+(\w+)', 'enum class'),
        (r'^\s*class\s+(\w+)', 'class'),
        (r'^\s*object\s+(\w+)', 'object'),
        (r'^\s*interface\s+(\w+)', 'interface'),
        (r'^\s*typealias\s+(\w+)', 'typealias'),  # Type aliases
        # Handle generic functions: fun <T> name(...) or fun name(...)
        (r'^\s*fun\s+(?:<[^>]+>\s+)?(\w+)', 'fun'),
        # Properties: with or without annotations before val/var
        (r'^\s*(?:@\w+(?:\([^)]*\))?\s+)*(?:val|var)\s+(\w+)\s*[=:]', 'property'),
        (r'^\s*const\s+val\s+(\w+)', 'const'),
        # Enum entries: with/without annotations, with comma, paren, semicolon, or alone
        (r'^\s*(?:@\w+(?:\([^)]*\))?\s+)*([A-Z][A-Za-z0-9]*)\s*[,\(;]', 'entry'),  # Annotated entry with punctuation
        (r'^\s*(?:@\w+(?:\([^)]*\))?\s+)+([A-Z][A-Za-z0-9]*)\s*$', 'entry'),  # Annotated final entry (no comma)
        (r'^\s*(\w+)\s*[,\(]', 'entry'),
        (r'^\s*([A-Z][A-Za-z0-9]*)\s*;', 'entry'),  # Enum entry followed by semicolon (before method)
        (r'^\s*([A-Z][A-Za-z0-9]*)\s*$', 'entry'),  # Standalone enum entry (last one, no comma)
        # Sealed class variants (data class or object)
        (r'^\s*(?:data\s+)?class\s+(\w+)\s*[:\(]', 'variant'),
        (r'^\s*(?:@\w+(?:\([^)]*\))?\s+)*object\s+(\w+)\s*:', 'variant'),  # object Foo : SealedClass()
    ]

    definitions: list[Definition] = []
    seen_names: set[str] = set()  # Avoid duplicates

    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            lines = f.readlines()

        for line_num, line in enumerate(lines, 1):
            # Skip suppressed definitions
            if has_portlint_suppression(lines, line_num):
                continue

            # Standard pattern matching (line-start patterns)
            for pattern, kind in patterns:
                match = re.match(pattern, line)
                if match:
                    name = match.group(1)
                    # Skip common false positives
                    if name in ('it', 'this', 'super', 'if', 'else', 'when', 'for', 'while', 'return', 'break', 'continue', 'is', 'as', 'in', 'val', 'var', 'fun', 'class', 'object'):
                        continue
                    normalized = NamingConverter.normalize(name)
                    if normalized not in seen_names:
                        seen_names.add(normalized)
                        definitions.append(Definition(
                            name=name,
                            kind=kind,
                            line=line_num,
                            context=line.strip(),
                            normalized=normalized
                        ))
                    break  # Only match one pattern per line

            # Extract constructor properties (val/var inside parentheses)
            # Matches: @Annotation(...) val name: Type or just val name: Type
            constructor_props = re.findall(r'(?:@\w+(?:\([^)]*\))?\s+)*(?:val|var)\s+(\w+)\s*:', line)
            for name in constructor_props:
                if name in ('it', 'this', 'super', 'if', 'else', 'when', 'for', 'while', 'return', 'break', 'continue', 'is', 'as', 'in', 'val', 'var', 'fun', 'class', 'object'):
                    continue
                normalized = NamingConverter.normalize(name)
                if normalized not in seen_names:
                    seen_names.add(normalized)
                    definitions.append(Definition(
                        name=name,
                        kind='property',
                        line=line_num,
                        context=line.strip()[:80] + ('...' if len(line.strip()) > 80 else ''),
                        normalized=normalized
                    ))
    except Exception:
        pass
    return definitions


@dataclass
class UnportedItem:
    """An item that needs to be ported."""
    rust_name: str
    rust_kind: str
    rust_line: int
    rust_file: Path
    kotlin_file: Optional[Path]  # None if file doesn't exist yet
    context: str
    priority: int  # Lower = higher priority (0 = file exists, 1 = file doesn't exist)

    def __lt__(self, other):
        # Sort by priority first, then by file path, then by line number
        if self.priority != other.priority:
            return self.priority < other.priority
        if str(self.rust_file) != str(other.rust_file):
            return str(self.rust_file) < str(other.rust_file)
        return self.rust_line < other.rust_line


def get_all_unported_items(linter: PortLinter) -> list[UnportedItem]:
    """Get all unported items, sorted by priority.

    Priority 0: Definitions missing from existing Kotlin files (finish what you started)
    Priority 1: Files that don't exist yet (new files to create)
    """
    items: list[UnportedItem] = []

    # Priority 0: Get definitions missing from matched files
    matched_pairs = linter.report_matched()
    for rust_node, kotlin_node in matched_pairs:
        rust_defs = extract_rust_file_definitions(rust_node.path)
        kotlin_defs = extract_kotlin_file_definitions(kotlin_node.path)

        # Build lookup by normalized name
        kotlin_by_norm: set[str] = {d.normalized for d in kotlin_defs}

        # Find Rust definitions missing from Kotlin
        for d in rust_defs:
            if d.normalized not in kotlin_by_norm:
                items.append(UnportedItem(
                    rust_name=d.name,
                    rust_kind=d.kind,
                    rust_line=d.line,
                    rust_file=rust_node.path,
                    kotlin_file=kotlin_node.path,
                    context=d.context,
                    priority=0
                ))

    # Priority 1: Get files that don't exist yet
    missing_files = linter.report_missing()
    for rust_node in missing_files:
        # Get all definitions from this Rust file
        rust_defs = extract_rust_file_definitions(rust_node.path)
        if rust_defs:
            # Just add the main definition (first struct/enum/trait)
            main_defs = [d for d in rust_defs if d.kind in ('struct', 'enum', 'trait', 'fn')]
            if main_defs:
                d = main_defs[0]
                items.append(UnportedItem(
                    rust_name=d.name,
                    rust_kind=d.kind,
                    rust_line=d.line,
                    rust_file=rust_node.path,
                    kotlin_file=None,
                    context=d.context,
                    priority=1
                ))
        else:
            # No definitions extracted, add file itself
            items.append(UnportedItem(
                rust_name=rust_node.name,
                rust_kind='file',
                rust_line=1,
                rust_file=rust_node.path,
                kotlin_file=None,
                context=f"// {rust_node.name}",
                priority=1
            ))

    items.sort()
    return items


def auto_mode(linter: PortLinter):
    """Show the first unported item and wait for it to be ported.

    Prioritizes:
    1. Definitions missing from existing Kotlin files (finish incomplete files first)
    2. Files that don't exist yet
    """
    items = get_all_unported_items(linter)

    if not items:
        print("\n" + "=" * 70)
        print("ALL DONE!")
        print("=" * 70)
        print("\nNo unported items found. The port is complete!")
        return

    item = items[0]
    remaining = len(items)

    print("\n" + "=" * 70)
    print("NEXT ITEM TO PORT")
    print("=" * 70)

    # Show priority context
    if item.priority == 0:
        print(f"\nPRIORITY: Finish existing file ({remaining} items remaining)")
        print(f"The Kotlin file exists but is missing this definition.\n")
    else:
        print(f"\nPRIORITY: New file needed ({remaining} items remaining)")
        print(f"No Kotlin file exists for this Rust file yet.\n")

    # Show the Rust definition
    try:
        rust_rel = item.rust_file.relative_to(linter.source_root)
    except ValueError:
        rust_rel = item.rust_file

    print(f"RUST SOURCE:")
    print(f"  File: {rust_rel}")
    print(f"  Line: {item.rust_line}")
    print(f"  Kind: {item.rust_kind}")
    print(f"  Name: {item.rust_name}")
    print(f"  Code: {item.context}")

    # Show where it should go in Kotlin
    print(f"\nKOTLIN TARGET:")
    if item.kotlin_file:
        try:
            kotlin_rel = item.kotlin_file.relative_to(linter.target_root)
        except ValueError:
            kotlin_rel = item.kotlin_file
        print(f"  File: {kotlin_rel}")
        print(f"  Action: Add {item.rust_kind} '{item.rust_name}'")

        # Suggest Kotlin name
        if item.rust_kind in ('struct', 'enum', 'trait'):
            kotlin_name = NamingConverter.snake_to_pascal(item.rust_name) if '_' in item.rust_name else item.rust_name
            print(f"  Kotlin name: {kotlin_name}")
        elif item.rust_kind == 'fn':
            kotlin_name = NamingConverter.snake_to_camel(item.rust_name) if '_' in item.rust_name else item.rust_name
            print(f"  Kotlin name: {kotlin_name}()")
    else:
        # Suggest new file location
        stem = Path(item.rust_file.name).stem
        kotlin_name = NamingConverter.snake_to_pascal(stem) if '_' in stem else stem
        print(f"  New file: {kotlin_name}.kt")
        print(f"  Action: Create file and port '{item.rust_name}'")

    # Show what's next
    if remaining > 1:
        print(f"\n{'─' * 70}")
        print(f"NEXT UP: {remaining - 1} more items")

        # Show preview of next few items
        preview_count = min(5, remaining - 1)
        for i, next_item in enumerate(items[1:preview_count + 1]):
            try:
                next_rust_rel = next_item.rust_file.relative_to(linter.source_root)
            except ValueError:
                next_rust_rel = next_item.rust_file
            status = "✓ file exists" if next_item.kotlin_file else "○ new file"
            print(f"  {i + 2}. {next_item.rust_kind} {next_item.rust_name} ({next_rust_rel}) [{status}]")

        if remaining - 1 > preview_count:
            print(f"  ... and {remaining - 1 - preview_count} more")

    print("\n" + "=" * 70)
    print("Run again after porting to see the next item.")
    print("=" * 70)


def deep_compare_matched_files(linter: PortLinter, missing_only: bool = False):
    """Compare definitions inside matched file pairs.

    For each matched (Rust, Kotlin) file pair:
    1. Extract all definitions from both files
    2. Match definitions by normalized name
    3. Report Rust definitions missing from Kotlin (need to port)
    4. Report Kotlin definitions not in Rust (invented/misplaced)
    """
    print("\n" + "=" * 70)
    print("DEEP FILE COMPARISON")
    print("Comparing definitions inside matched file pairs")
    print("=" * 70)

    matched_pairs = linter.report_matched()
    if not matched_pairs:
        print("\nNo matched file pairs to compare.")
        return

    total_rust_defs = 0
    total_kotlin_defs = 0
    total_matched = 0
    total_rust_only = 0
    total_kotlin_only = 0

    files_with_missing: list[tuple[Path, Path, list[Definition], list[Definition]]] = []

    for rust_node, kotlin_node in matched_pairs:
        rust_defs = extract_rust_file_definitions(rust_node.path)
        kotlin_defs = extract_kotlin_file_definitions(kotlin_node.path)

        total_rust_defs += len(rust_defs)
        total_kotlin_defs += len(kotlin_defs)

        # Build lookup by normalized name
        kotlin_by_norm: dict[str, list[Definition]] = {}
        for d in kotlin_defs:
            if d.normalized not in kotlin_by_norm:
                kotlin_by_norm[d.normalized] = []
            kotlin_by_norm[d.normalized].append(d)

        rust_by_norm: dict[str, list[Definition]] = {}
        for d in rust_defs:
            if d.normalized not in rust_by_norm:
                rust_by_norm[d.normalized] = []
            rust_by_norm[d.normalized].append(d)

        # Find Rust definitions missing from Kotlin
        rust_only: list[Definition] = []
        matched_rust_norms: set[str] = set()
        for d in rust_defs:
            if d.normalized in kotlin_by_norm:
                matched_rust_norms.add(d.normalized)
                total_matched += 1
            else:
                rust_only.append(d)
                total_rust_only += 1

        # Find Kotlin definitions not in Rust
        kotlin_only: list[Definition] = []
        for d in kotlin_defs:
            if d.normalized not in rust_by_norm:
                kotlin_only.append(d)
                total_kotlin_only += 1

        if rust_only or kotlin_only:
            files_with_missing.append((rust_node.path, kotlin_node.path, rust_only, kotlin_only))

    # Print results
    if files_with_missing:
        print(f"\n{'─' * 70}")
        print(f"FILES WITH MISSING/EXTRA DEFINITIONS ({len(files_with_missing)} files)")
        print(f"{'─' * 70}")

        for rust_path, kotlin_path, rust_only, kotlin_only in files_with_missing:
            rust_rel = rust_path.relative_to(linter.source_root)
            kotlin_rel = kotlin_path.relative_to(linter.target_root)

            # Skip if missing_only and no rust_only items
            if missing_only and not rust_only:
                continue

            print(f"\n  {rust_rel} <-> {kotlin_rel}")

            if rust_only:
                print(f"    MISSING FROM KOTLIN ({len(rust_only)}):")
                # Group by kind
                by_kind: dict[str, list[Definition]] = {}
                for d in rust_only:
                    if d.kind not in by_kind:
                        by_kind[d.kind] = []
                    by_kind[d.kind].append(d)

                for kind in sorted(by_kind.keys()):
                    items = by_kind[kind]
                    print(f"      {kind}:")
                    for d in sorted(items, key=lambda x: x.line)[:15]:
                        print(f"        {d.line:5}: {d.name}")
                    if len(items) > 15:
                        print(f"        ... and {len(items) - 15} more")

            if kotlin_only and not missing_only:
                print(f"    KOTLIN-ONLY ({len(kotlin_only)}):")
                # Group by kind
                by_kind: dict[str, list[Definition]] = {}
                for d in kotlin_only:
                    if d.kind not in by_kind:
                        by_kind[d.kind] = []
                    by_kind[d.kind].append(d)

                for kind in sorted(by_kind.keys()):
                    items = by_kind[kind]
                    print(f"      {kind}:")
                    for d in sorted(items, key=lambda x: x.line)[:10]:
                        print(f"        {d.line:5}: {d.name}")
                    if len(items) > 10:
                        print(f"        ... and {len(items) - 10} more")

    # Summary
    print(f"\n{'─' * 70}")
    print("DEEP COMPARISON SUMMARY:")
    print(f"  Files compared:      {len(matched_pairs)}")
    print(f"  Rust definitions:    {total_rust_defs}")
    print(f"  Kotlin definitions:  {total_kotlin_defs}")
    print(f"  Matched:             {total_matched}")
    print(f"  Missing from Kotlin: {total_rust_only} (need to port)")
    print(f"  Kotlin-only:         {total_kotlin_only} (invented/extra)")
    if total_rust_defs > 0:
        print(f"  Coverage:            {total_matched / total_rust_defs * 100:.1f}%")
    print("=" * 70)


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
    suppressed_count = 0

    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue

        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                lines = f.readlines()

            for line_num, line in enumerate(lines, 1):
                for pattern, kind in patterns:
                    match = re.match(pattern, line)
                    if match:
                        # Check for suppression comment
                        if has_portlint_suppression(lines, line_num):
                            suppressed_count += 1
                            continue

                        # Get the name (last group)
                        name = match.groups()[-1]
                        key = f"{kind}:{name}"
                        if key not in definitions:
                            definitions[key] = []
                        definitions[key].append((kt_file, line_num, line.strip()))
        except Exception:
            pass

    if suppressed_count > 0:
        print(f"  Suppressed {suppressed_count} definitions via // port-lint: ignore")

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
    # Re-use the same suppression logic
    all_kotlin_defs: dict[str, list[tuple[Path, int, str]]] = {}
    for kt_file in kotlin_root.rglob("*.kt"):
        if any(skip in str(kt_file) for skip in ['build', '.gradle', '.git']):
            continue
        try:
            with open(kt_file, 'r', encoding='utf-8', errors='ignore') as f:
                lines = f.readlines()

            for line_num, line in enumerate(lines, 1):
                for pattern, kind in patterns:
                    match = re.match(pattern, line)
                    if match:
                        # Skip suppressed definitions
                        if has_portlint_suppression(lines, line_num):
                            continue

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
        rust_only: list[tuple[str, Path, int]] = []
        matched: list[tuple[str, str, float]] = []
        matched_rust_names: set[str] = set()

        # Find Kotlin names and their Rust matches
        for kt_name, locations in kotlin_index.names.items():
            rust_matches = rust_index.find_similar(kt_name, threshold=threshold, top_k=1)
            if rust_matches:
                matched.append((kt_name, rust_matches[0].name, rust_matches[0].score))
                matched_rust_names.add(rust_matches[0].name)
            else:
                for loc in locations:
                    kotlin_only.append((kt_name, loc[0], loc[1]))

        # Find Rust names without Kotlin equivalents (WHAT'S LEFT TO PORT)
        for rust_name, locations in rust_index.names.items():
            if rust_name not in matched_rust_names:
                # Double-check with reverse lookup
                kotlin_matches = kotlin_index.find_similar(rust_name, threshold=threshold, top_k=1)
                if not kotlin_matches:
                    for loc in locations:
                        rust_only.append((rust_name, loc[0], loc[1]))

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

        # Show unmatched Rust names (WHAT'S LEFT TO PORT)
        if rust_only:
            print(f"\n{'─' * 70}")
            print(f"RUST TYPES/FUNCTIONS TO PORT ({len(rust_only)})")
            print("These Rust definitions need Kotlin equivalents")
            print(f"{'─' * 70}")

            # Group by file for readability
            by_file: dict[Path, list[tuple[str, int]]] = {}
            for name, path, line in rust_only:
                if path not in by_file:
                    by_file[path] = []
                by_file[path].append((name, line))

            for path in sorted(by_file.keys(), key=lambda p: str(p)):
                rel_path = path.relative_to(rust_root)
                items = by_file[path]
                print(f"\n  {rel_path}:")
                for name, line in sorted(items, key=lambda x: x[1])[:20]:
                    print(f"    {line:5}: {name}")
                if len(items) > 20:
                    print(f"    ... and {len(items) - 20} more")

        print(f"\n{'─' * 70}")
        print("SUMMARY:")
        print(f"  Matched pairs:     {len(matched)}")
        print(f"  Unmatched Rust:    {len(rust_only)} (need to port)")
        print(f"  Unmatched Kotlin:  {len(kotlin_only)} (invented/extra)")
        total_rust = len(matched) + len(rust_only)
        if total_rust > 0:
            print(f"  Port coverage:     {len(matched) / total_rust * 100:.1f}%")

    print("\n" + "=" * 70)


def run_compile_check(kotlin_root: Path, verbose: bool = False):
    """Run Kotlin compiler and report errors/warnings.

    Uses gradle to compile the project and parses the output to identify:
    - Unresolved references (missing types, functions)
    - Type mismatches
    - Deprecated usage
    - Naming convention violations (snake_case vs camelCase)
    """
    import subprocess

    print("\n" + "=" * 70)
    print("KOTLIN COMPILE CHECK")
    print("Running Kotlin compiler to detect real errors...")
    print("=" * 70)

    # Find project root (parent of src directory)
    project_root = kotlin_root.parent
    gradle_wrapper = project_root / "gradlew"

    if not gradle_wrapper.exists():
        print(f"\nError: gradlew not found at {gradle_wrapper}")
        print("Please ensure you're in a Gradle project.")
        return

    # Run gradle compile - use metadata compilation for cross-platform code
    compile_task = "compileNativeMainKotlinMetadata"
    print(f"\nRunning: ./gradlew {compile_task}")
    print("(This may take a moment...)\n")

    try:
        result = subprocess.run(
            ["./gradlew", compile_task, "--no-daemon"],
            cwd=project_root,
            capture_output=True,
            text=True,
            timeout=300  # 5 minute timeout
        )
        output = result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        print("Error: Compilation timed out after 5 minutes")
        return
    except Exception as e:
        print(f"Error running gradle: {e}")
        return

    # Parse compiler output
    # Format: e: file:///path/to/file.kt:47:9 Error message (note: space, not colon before message)
    # Format: w: file:///path/to/file.kt:22:8 Warning message
    error_pattern = re.compile(r'^([ew]): file:///(.+?):(\d+):(\d+)\s+(.+)$', re.MULTILINE)

    errors: list[tuple[str, Path, int, int, str]] = []  # (type, file, line, col, message)
    warnings: list[tuple[str, Path, int, int, str]] = []

    for match in error_pattern.finditer(output):
        level, file_path, line, col, message = match.groups()
        # Re-add leading slash for absolute path
        path = Path("/" + file_path)
        line_num = int(line)
        col_num = int(col)

        if level == 'e':
            errors.append(('error', path, line_num, col_num, message))
        else:
            warnings.append(('warning', path, line_num, col_num, message))

    # Categorize errors by type
    unresolved_refs: list[tuple[Path, int, int, str]] = []
    type_mismatches: list[tuple[Path, int, int, str]] = []
    naming_issues: list[tuple[Path, int, int, str]] = []  # snake_case detection
    other_errors: list[tuple[Path, int, int, str]] = []

    for level, path, line, col, message in errors:
        if 'Unresolved reference' in message or 'None of the following candidates' in message:
            unresolved_refs.append((path, line, col, message))
        elif 'type mismatch' in message.lower() or 'required:' in message.lower():
            type_mismatches.append((path, line, col, message))
        elif '_' in message and ('camelCase' in message or 'parameter' in message.lower()):
            naming_issues.append((path, line, col, message))
        else:
            other_errors.append((path, line, col, message))

    # Also check for snake_case in parameter names being used incorrectly
    snake_case_in_errors = []
    for level, path, line, col, message in errors:
        # Look for patterns like "call_id" when "callId" is expected
        snake_match = re.search(r"'([a-z]+_[a-z_]+)'", message)
        if snake_match:
            snake_name = snake_match.group(1)
            # Convert to camelCase
            parts = snake_name.split('_')
            camel_name = parts[0] + ''.join(p.capitalize() for p in parts[1:])
            snake_case_in_errors.append((path, line, col, message, snake_name, camel_name))

    # Report results
    if errors:
        print(f"{'─' * 70}")
        print(f"COMPILATION ERRORS ({len(errors)})")
        print(f"{'─' * 70}")

        if unresolved_refs:
            print(f"\n  UNRESOLVED REFERENCES ({len(unresolved_refs)}):")
            print("  (These are missing types/functions that need to be ported or imported)")
            by_file: dict[Path, list] = {}
            for path, line, col, msg in unresolved_refs:
                if path not in by_file:
                    by_file[path] = []
                by_file[path].append((line, col, msg))

            for path in sorted(by_file.keys(), key=lambda p: str(p)):
                try:
                    rel_path = path.relative_to(kotlin_root)
                except ValueError:
                    rel_path = path
                print(f"\n    {rel_path}:")
                for line, col, msg in sorted(by_file[path], key=lambda x: x[0])[:10]:
                    # Extract the unresolved name from the message
                    ref_match = re.search(r"Unresolved reference[:\s]*'?(\w+)'?", msg)
                    if ref_match:
                        ref_name = ref_match.group(1)
                        print(f"      {line:4}:{col:<3} -> {ref_name}")
                    else:
                        print(f"      {line:4}:{col:<3} {msg[:60]}...")
                if len(by_file[path]) > 10:
                    print(f"      ... and {len(by_file[path]) - 10} more")

        if snake_case_in_errors:
            print(f"\n  SNAKE_CASE NAMING ISSUES ({len(snake_case_in_errors)}):")
            print("  (Property names using snake_case but declared as camelCase)")
            seen = set()
            for path, line, col, msg, snake, camel in snake_case_in_errors:
                if snake not in seen:
                    seen.add(snake)
                    print(f"    {snake:30} should be {camel}")

        if type_mismatches:
            print(f"\n  TYPE MISMATCHES ({len(type_mismatches)}):")
            for path, line, col, msg in type_mismatches[:10]:
                try:
                    rel_path = path.relative_to(kotlin_root)
                except ValueError:
                    rel_path = path
                print(f"    {rel_path}:{line}:{col}")
                print(f"      {msg[:80]}...")
            if len(type_mismatches) > 10:
                print(f"    ... and {len(type_mismatches) - 10} more")

        if other_errors and verbose:
            print(f"\n  OTHER ERRORS ({len(other_errors)}):")
            for path, line, col, msg in other_errors[:20]:
                try:
                    rel_path = path.relative_to(kotlin_root)
                except ValueError:
                    rel_path = path
                print(f"    {rel_path}:{line}:{col}")
                print(f"      {msg[:80]}...")
            if len(other_errors) > 20:
                print(f"    ... and {len(other_errors) - 20} more")

    # Warnings
    if warnings:
        deprecated_warnings = [(p, l, c, m) for p, l, c, m in warnings if 'deprecated' in m.lower()]
        unused_warnings = [(p, l, c, m) for p, l, c, m in warnings if 'unused' in m.lower() or 'never used' in m.lower()]
        other_warnings = [(p, l, c, m) for p, l, c, m in warnings
                          if 'deprecated' not in m.lower() and 'unused' not in m.lower() and 'never used' not in m.lower()]

        print(f"\n{'─' * 70}")
        print(f"WARNINGS ({len(warnings)})")
        print(f"{'─' * 70}")

        if deprecated_warnings:
            print(f"\n  DEPRECATED USAGE ({len(deprecated_warnings)}):")
            seen_deps = set()
            for path, line, col, msg in deprecated_warnings:
                dep_match = re.search(r"'(\w+)' is deprecated", msg)
                if dep_match:
                    dep_name = dep_match.group(1)
                    if dep_name not in seen_deps:
                        seen_deps.add(dep_name)
                        print(f"    {dep_name}")

        if unused_warnings and verbose:
            print(f"\n  UNUSED DECLARATIONS ({len(unused_warnings)}):")
            for path, line, col, msg in unused_warnings[:15]:
                try:
                    rel_path = path.relative_to(kotlin_root)
                except ValueError:
                    rel_path = path
                # Extract what's unused
                unused_match = re.search(r"(Parameter|Variable|Property|Function|Import)\s+'(\w+)'", msg, re.IGNORECASE)
                if unused_match:
                    kind, name = unused_match.groups()
                    print(f"    {kind:10} {name:30} ({rel_path}:{line})")
                else:
                    print(f"    {rel_path}:{line}: {msg[:50]}...")
            if len(unused_warnings) > 15:
                print(f"    ... and {len(unused_warnings) - 15} more")
        elif unused_warnings:
            print(f"\n  UNUSED DECLARATIONS: {len(unused_warnings)} (use --verbose to see)")

        if other_warnings and verbose:
            print(f"\n  OTHER WARNINGS ({len(other_warnings)}):")
            for path, line, col, msg in other_warnings[:10]:
                try:
                    rel_path = path.relative_to(kotlin_root)
                except ValueError:
                    rel_path = path
                print(f"    {rel_path}:{line}:{col}: {msg[:60]}...")
            if len(other_warnings) > 10:
                print(f"    ... and {len(other_warnings) - 10} more")

    # Summary
    print(f"\n{'─' * 70}")
    print("COMPILE CHECK SUMMARY:")
    print(f"  Total errors:      {len(errors)}")
    if errors:
        print(f"    Unresolved refs:   {len(unresolved_refs)}")
        print(f"    Type mismatches:   {len(type_mismatches)}")
        print(f"    Naming issues:     {len(snake_case_in_errors)}")
        print(f"    Other:             {len(other_errors)}")
    print(f"  Total warnings:    {len(warnings)}")

    if result.returncode == 0:
        print("\n  Build: SUCCESS")
    else:
        print("\n  Build: FAILED")

    print("=" * 70)

    # Return counts for programmatic use
    return {
        'errors': len(errors),
        'warnings': len(warnings),
        'unresolved_refs': len(unresolved_refs),
        'type_mismatches': len(type_mismatches),
        'naming_issues': len(snake_case_in_errors),
        'success': result.returncode == 0
    }


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Port Linter - Compare codebases for porting (Rust, Kotlin, TypeScript)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
MODES:
  --auto              Show ONE unported item at a time (prioritizes existing files)
  --compile-check     Run Kotlin compiler to detect real errors (missing types, naming issues)
  --compare           Compare FILE structures between codebases
  --deep              Compare DEFINITIONS inside matched files (classes, functions, etc.)
  --similar           Find similar names using cosine similarity (shows what's left to port)
  --duplicates        Find duplicate definitions in Kotlin codebase
  --snake-case        Check for snake_case violations in Kotlin code
  --search SYMBOL     Search for a specific symbol in both codebases

FILTERS:
  --focus DIR         Focus on specific directory (e.g., core, protocol)
  --missing-only      Only show items missing from Kotlin (hide Kotlin-only)
  --matched           Show matched files (with --compare)
  --verbose           Show detailed output

WORKFLOW:
  The --auto mode is ideal for incremental porting. It shows one item at a time,
  prioritizing definitions missing from existing Kotlin files (finish what you
  started) over creating new files. Run it repeatedly to work through the list.

EXAMPLES:
  # Work through items one at a time (recommended workflow):
  %(prog)s --auto --focus protocol

  # Run the compiler to find real errors:
  %(prog)s --compile-check
  %(prog)s --compile-check --verbose  # Include unused warnings

  # What files need porting?
  %(prog)s --compare --focus protocol

  # What definitions are missing inside matched files?
  %(prog)s --deep --focus protocol

  # Show only what's left to port (definitions):
  %(prog)s --deep --missing-only --focus protocol

  # What functions/types need porting? (global similarity search)
  %(prog)s --similar --focus protocol

  # Search for a specific symbol:
  %(prog)s --search StreamInfoEvent

  # Find duplicate type definitions:
  %(prog)s --duplicates

  # Check for snake_case violations:
  %(prog)s --snake-case
        """
    )

    # Paths
    parser.add_argument('--rust-root', type=Path,
                        default=Path(__file__).parent.parent / 'codex-rs',
                        help='Root of Rust codebase')
    parser.add_argument('--kotlin-root', type=Path,
                        default=Path(__file__).parent.parent / 'src',
                        help='Root of Kotlin codebase')
    parser.add_argument('--ts-root', type=Path,
                        help='Root of TypeScript codebase')

    # Porting mode
    parser.add_argument('--mode', type=str, default='rust2kotlin',
                        choices=['rust2kotlin', 'ts2rust'],
                        help='Porting direction (default: rust2kotlin)')

    # Modes
    parser.add_argument('--auto', action='store_true',
                        help='Show ONE unported item at a time (prioritizes existing files)')
    parser.add_argument('--compile-check', action='store_true',
                        help='Run Kotlin compiler to check for errors')
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
    parser.add_argument('--deep', action='store_true',
                        help='Deep compare definitions inside matched files')
    parser.add_argument('--missing-only', action='store_true',
                        help='Only show items missing from Kotlin (hide Kotlin-only)')

    args = parser.parse_args()

    # Determine source and target based on mode
    if args.mode == 'ts2rust':
        if not args.ts_root:
            print("Error: --ts-root required for ts2rust mode", file=sys.stderr)
            sys.exit(1)
        if not args.ts_root.exists():
            print(f"Error: TypeScript root not found: {args.ts_root}", file=sys.stderr)
            sys.exit(1)
        if not args.rust_root.exists():
            print(f"Error: Rust root not found: {args.rust_root}", file=sys.stderr)
            sys.exit(1)
        source_root = args.ts_root
        source_lang = 'typescript'
        target_root = args.rust_root
        target_lang = 'rust'
    else:  # rust2kotlin (default)
        if not args.rust_root.exists():
            print(f"Error: Rust root not found: {args.rust_root}", file=sys.stderr)
            sys.exit(1)
        if not args.kotlin_root.exists():
            print(f"Error: Kotlin root not found: {args.kotlin_root}", file=sys.stderr)
            sys.exit(1)
        source_root = args.rust_root
        source_lang = 'rust'
        target_root = args.kotlin_root
        target_lang = 'kotlin'

    # Execute requested mode
    if args.auto:
        # Auto mode: show one unported item at a time
        focus_root = source_root
        if args.focus:
            focused = source_root / args.focus
            if focused.exists():
                focus_root = focused
            else:
                print(f"Warning: --focus directory not found: {focused}")

        linter = PortLinter(focus_root, target_root, source_lang, target_lang)
        linter.analyze()
        auto_mode(linter)
    elif args.compile_check:
        # Run Kotlin compiler to check for real errors
        run_compile_check(target_root, verbose=args.verbose)
    elif args.similar or args.similar_query:
        find_similar_names(source_root, target_root,
                          threshold=args.threshold, query=args.similar_query)
    elif args.snake_case:
        # Get source definitions for cross-reference
        if source_lang == 'rust':
            source_defs = find_rust_definitions(source_root)
        else:
            source_defs = None  # TODO: add TypeScript definition finder
        check_snake_case_in_kotlin(target_root, source_defs)
    elif args.duplicates:
        print_duplicates_report(target_root, source_root)
    elif args.deep:
        # Deep comparison of definitions inside matched files
        focus_root = source_root
        if args.focus:
            focused = source_root / args.focus
            if focused.exists():
                focus_root = focused
            else:
                print(f"Warning: --focus directory not found: {focused}")

        linter = PortLinter(focus_root, target_root, source_lang, target_lang)
        linter.analyze()
        deep_compare_matched_files(linter, missing_only=args.missing_only)
    elif args.search:
        search_symbol(source_root, target_root, args.search)
    elif args.compare:
        focus_root = source_root
        if args.focus:
            focused = source_root / args.focus
            if focused.exists():
                focus_root = focused
            else:
                print(f"Warning: --focus directory not found: {focused}")

        linter = PortLinter(focus_root, target_root, source_lang, target_lang)
        linter.analyze()
        linter.print_report(verbose=args.verbose)

        if args.matched:
            # Build lookup for match type
            annotation_set = {(kt.path, rs.path) for kt, rs, _ in linter._annotation_matches} if hasattr(linter, '_annotation_matches') else set()
            namespace_set = {(kt.path, rs.path) for kt, rs in linter._namespace_matches} if hasattr(linter, '_namespace_matches') else set()

            matched = linter.report_matched()
            print(f"\n{'─' * 70}")
            print(f"MATCHED FILES ({len(matched)}):")
            print(f"{'─' * 70}")

            # Group by match type
            by_annotation = []
            by_namespace = []
            by_name = []

            for rust_node, kotlin_node in matched:
                key = (kotlin_node.path, rust_node.path)
                if key in annotation_set:
                    by_annotation.append((rust_node, kotlin_node))
                elif key in namespace_set:
                    by_namespace.append((rust_node, kotlin_node))
                else:
                    by_name.append((rust_node, kotlin_node))

            if by_annotation:
                print(f"\n  VIA SOURCE ANNOTATION ({len(by_annotation)}):")
                for rust_node, kotlin_node in sorted(by_annotation, key=lambda p: p[0].name):
                    source = linter.target_tree.get_source_annotation(kotlin_node)
                    print(f"    {kotlin_node.name:30} <- {source}")

            if by_namespace:
                print(f"\n  VIA NAMESPACE PATH ({len(by_namespace)}):")
                for rust_node, kotlin_node in sorted(by_namespace, key=lambda p: p[0].name):
                    ns = rust_node.namespace_path(linter.rust_root, 'rust')
                    print(f"    {ns}")
                    print(f"      {rust_node.name:28} <-> {kotlin_node.name}")

            if by_name:
                print(f"\n  VIA FILENAME ONLY ({len(by_name)}) - consider adding source annotations:")
                for rust_node, kotlin_node in sorted(by_name, key=lambda p: p[0].name):
                    rust_ns = rust_node.namespace_path(linter.rust_root, 'rust')
                    kotlin_ns = kotlin_node.namespace_path(linter.kotlin_root, 'kotlin')
                    print(f"    {rust_node.name:40} <-> {kotlin_node.name}")
                    print(f"      rust:   {rust_ns}")
                    print(f"      kotlin: {kotlin_ns}")
                    # Suggest annotation
                    try:
                        rust_rel = rust_node.path.relative_to(linter.rust_root)
                        print(f"      add: // port-lint: source {rust_rel}")
                    except ValueError:
                        pass
    else:
        # Default: show comparison
        linter = PortLinter(args.rust_root, args.kotlin_root)
        linter.analyze()
        linter.print_report(verbose=args.verbose)


if __name__ == '__main__':
    main()
