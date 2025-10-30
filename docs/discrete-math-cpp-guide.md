# Discrete Mathematics and C++ Programming: A Comprehensive Guide

## Table of Contents
1. [Introduction](#introduction)
2. [Logic and Boolean Algebra](#logic-and-boolean-algebra)
3. [Set Theory](#set-theory)
4. [Functions and Relations](#functions-and-relations)
5. [Combinatorics](#combinatorics)
6. [Graph Theory](#graph-theory)
7. [Number Theory](#number-theory)
8. [Trees and Hierarchical Structures](#trees-and-hierarchical-structures)
9. [Recursion and Mathematical Induction](#recursion-and-mathematical-induction)
10. [Algorithm Analysis and Complexity](#algorithm-analysis-and-complexity)
11. [Practical Applications](#practical-applications)

---

## Introduction

Discrete mathematics is the foundation of computer science and programming. Unlike continuous mathematics (calculus, analysis), discrete math deals with distinct, separate values - exactly how computers operate with discrete bits and finite states.

**Why Discrete Math Matters for C++ Programming:**
- **Data Structures**: Trees, graphs, and hash tables are discrete structures
- **Algorithm Design**: Sorting, searching, and optimization rely on discrete math principles
- **Logic**: Boolean algebra directly maps to conditional statements and bitwise operations
- **Complexity Analysis**: Big-O notation comes from discrete math
- **Combinatorics**: Essential for permutations, subsets, and counting problems
- **Graph Algorithms**: Social networks, routing, and dependency resolution

---

## Logic and Boolean Algebra

### Mathematical Foundation

Boolean algebra operates on binary values (true/false, 1/0) with operations:
- **AND** (∧): conjunction
- **OR** (∨): disjunction
- **NOT** (¬): negation
- **XOR** (⊕): exclusive or
- **IMPLIES** (→): implication

### C++ Implementation

```cpp
#include <iostream>
#include <bitset>

// Boolean algebra operations
bool implies(bool p, bool q) {
    return !p || q;  // p → q ≡ ¬p ∨ q
}

bool biconditional(bool p, bool q) {
    return (p && q) || (!p && !q);  // p ↔ q
}

// Truth table generator
void truthTable() {
    std::cout << "P\tQ\tAND\tOR\tXOR\tIMPLIES\n";
    for (int p = 0; p <= 1; p++) {
        for (int q = 0; q <= 1; q++) {
            std::cout << p << "\t" << q << "\t"
                      << (p && q) << "\t"
                      << (p || q) << "\t"
                      << (p ^ q) << "\t"
                      << implies(p, q) << "\n";
        }
    }
}

// Bitwise operations (Boolean algebra on multiple bits)
void bitwiseOperations() {
    unsigned int a = 0b1010;  // 10 in decimal
    unsigned int b = 0b1100;  // 12 in decimal

    std::cout << "AND: " << std::bitset<4>(a & b) << "\n";  // 1000
    std::cout << "OR:  " << std::bitset<4>(a | b) << "\n";  // 1110
    std::cout << "XOR: " << std::bitset<4>(a ^ b) << "\n";  // 0110
    std::cout << "NOT: " << std::bitset<4>(~a) << "\n";     // 0101
}
```

### Practical Applications

1. **Conditional Logic**: All `if`, `while`, and `for` statements use Boolean algebra
2. **Bit Flags**: Using bitwise operations for efficient state management
3. **Circuit Design**: Boolean logic maps directly to hardware gates
4. **Compiler Optimization**: Short-circuit evaluation and dead code elimination

```cpp
// Example: Permission system using bit flags
enum Permissions {
    READ    = 1 << 0,  // 0001
    WRITE   = 1 << 1,  // 0010
    EXECUTE = 1 << 2,  // 0100
    DELETE  = 1 << 3   // 1000
};

class FilePermissions {
    unsigned int perms;
public:
    FilePermissions() : perms(0) {}

    void grant(Permissions p) { perms |= p; }
    void revoke(Permissions p) { perms &= ~p; }
    bool has(Permissions p) const { return (perms & p) != 0; }
};
```

---

## Set Theory

### Mathematical Foundation

A set is an unordered collection of distinct elements. Key operations:
- **Union** (A ∪ B): elements in A or B
- **Intersection** (A ∩ B): elements in both A and B
- **Difference** (A - B): elements in A but not in B
- **Subset** (A ⊆ B): all elements of A are in B
- **Power Set** (P(A)): set of all subsets of A
- **Cartesian Product** (A × B): all ordered pairs (a,b)

### C++ Implementation

```cpp
#include <set>
#include <algorithm>
#include <vector>
#include <iostream>

template<typename T>
class DiscreteSet {
    std::set<T> elements;

public:
    DiscreteSet() = default;
    DiscreteSet(std::initializer_list<T> init) : elements(init) {}

    // Basic operations
    void insert(const T& elem) { elements.insert(elem); }
    void remove(const T& elem) { elements.erase(elem); }
    bool contains(const T& elem) const { return elements.count(elem) > 0; }
    size_t cardinality() const { return elements.size(); }

    // Set operations
    DiscreteSet setUnion(const DiscreteSet& other) const {
        DiscreteSet result;
        std::set_union(elements.begin(), elements.end(),
                      other.elements.begin(), other.elements.end(),
                      std::inserter(result.elements, result.elements.begin()));
        return result;
    }

    DiscreteSet intersection(const DiscreteSet& other) const {
        DiscreteSet result;
        std::set_intersection(elements.begin(), elements.end(),
                             other.elements.begin(), other.elements.end(),
                             std::inserter(result.elements, result.elements.begin()));
        return result;
    }

    DiscreteSet difference(const DiscreteSet& other) const {
        DiscreteSet result;
        std::set_difference(elements.begin(), elements.end(),
                           other.elements.begin(), other.elements.end(),
                           std::inserter(result.elements, result.elements.begin()));
        return result;
    }

    bool isSubsetOf(const DiscreteSet& other) const {
        return std::includes(other.elements.begin(), other.elements.end(),
                           elements.begin(), elements.end());
    }

    // Generate power set (set of all subsets)
    std::vector<DiscreteSet> powerSet() const {
        std::vector<T> vec(elements.begin(), elements.end());
        std::vector<DiscreteSet> result;
        size_t n = vec.size();
        size_t power = 1 << n;  // 2^n subsets

        for (size_t i = 0; i < power; i++) {
            DiscreteSet subset;
            for (size_t j = 0; j < n; j++) {
                if (i & (1 << j)) {
                    subset.insert(vec[j]);
                }
            }
            result.push_back(subset);
        }
        return result;
    }

    void print() const {
        std::cout << "{";
        bool first = true;
        for (const auto& elem : elements) {
            if (!first) std::cout << ", ";
            std::cout << elem;
            first = false;
        }
        std::cout << "}\n";
    }
};
```

### Practical Applications

1. **Hash Sets/Maps**: Fast membership testing O(1)
2. **Unique Collections**: Removing duplicates from data
3. **Database Operations**: SQL JOIN, UNION, INTERSECT
4. **Access Control**: User roles and permissions
5. **Tag Systems**: Content categorization

---

## Functions and Relations

### Mathematical Foundation

- **Function**: A mapping f: A → B where each input has exactly one output
- **Relation**: A subset of A × B (Cartesian product)
- **Properties**: Reflexive, symmetric, transitive, antisymmetric
- **Equivalence Relation**: Reflexive, symmetric, and transitive
- **Partial Order**: Reflexive, antisymmetric, and transitive

### C++ Implementation

```cpp
#include <map>
#include <functional>
#include <vector>
#include <set>

// Function representation
template<typename Domain, typename Codomain>
class MathFunction {
    std::map<Domain, Codomain> mapping;

public:
    void define(const Domain& input, const Codomain& output) {
        mapping[input] = output;
    }

    Codomain apply(const Domain& input) const {
        auto it = mapping.find(input);
        if (it != mapping.end()) {
            return it->second;
        }
        throw std::runtime_error("Function not defined for this input");
    }

    bool isDefined(const Domain& input) const {
        return mapping.count(input) > 0;
    }

    // Check if function is injective (one-to-one)
    bool isInjective() const {
        std::set<Codomain> outputs;
        for (const auto& pair : mapping) {
            if (outputs.count(pair.second) > 0) {
                return false;  // Found duplicate output
            }
            outputs.insert(pair.second);
        }
        return true;
    }

    // Compose two functions: (g ∘ f)(x) = g(f(x))
    template<typename NewCodomain>
    MathFunction<Domain, NewCodomain> compose(
        const MathFunction<Codomain, NewCodomain>& g) const {
        MathFunction<Domain, NewCodomain> result;
        for (const auto& [input, output] : mapping) {
            if (g.isDefined(output)) {
                result.define(input, g.apply(output));
            }
        }
        return result;
    }
};

// Binary relation
template<typename T>
class Relation {
    std::set<std::pair<T, T>> pairs;

public:
    void add(const T& a, const T& b) {
        pairs.insert({a, b});
    }

    bool contains(const T& a, const T& b) const {
        return pairs.count({a, b}) > 0;
    }

    // Check if reflexive: (a,a) for all a
    bool isReflexive(const std::set<T>& domain) const {
        for (const auto& elem : domain) {
            if (!contains(elem, elem)) return false;
        }
        return true;
    }

    // Check if symmetric: (a,b) implies (b,a)
    bool isSymmetric() const {
        for (const auto& [a, b] : pairs) {
            if (!contains(b, a)) return false;
        }
        return true;
    }

    // Check if transitive: (a,b) and (b,c) implies (a,c)
    bool isTransitive() const {
        for (const auto& [a, b] : pairs) {
            for (const auto& [c, d] : pairs) {
                if (b == c && !contains(a, d)) {
                    return false;
                }
            }
        }
        return true;
    }

    // Check if antisymmetric: (a,b) and (b,a) implies a=b
    bool isAntisymmetric() const {
        for (const auto& [a, b] : pairs) {
            if (a != b && contains(b, a)) {
                return false;
            }
        }
        return true;
    }
};
```

### Practical Applications

1. **Hash Functions**: Mapping keys to array indices
2. **Lambda Functions**: C++ functional programming
3. **Database Relations**: Tables represent relations
4. **Equivalence Classes**: Union-Find data structure
5. **Partial Orders**: Dependency graphs, topological sorting

---

## Combinatorics

### Mathematical Foundation

Combinatorics studies counting, arrangement, and selection:
- **Permutations**: Arrangements where order matters (n!)
- **Combinations**: Selections where order doesn't matter (C(n,k))
- **Binomial Coefficient**: C(n,k) = n! / (k!(n-k)!)
- **Pigeonhole Principle**: If n items in m containers, some container has ⌈n/m⌉ items

### C++ Implementation

```cpp
#include <vector>
#include <algorithm>
#include <cmath>

// Factorial
unsigned long long factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

// Binomial coefficient C(n, k)
unsigned long long binomial(int n, int k) {
    if (k > n) return 0;
    if (k == 0 || k == n) return 1;
    if (k > n - k) k = n - k;  // Optimization: C(n,k) = C(n,n-k)

    unsigned long long result = 1;
    for (int i = 0; i < k; i++) {
        result *= (n - i);
        result /= (i + 1);
    }
    return result;
}

// Generate all permutations
template<typename T>
std::vector<std::vector<T>> generatePermutations(std::vector<T> elements) {
    std::vector<std::vector<T>> result;
    std::sort(elements.begin(), elements.end());

    do {
        result.push_back(elements);
    } while (std::next_permutation(elements.begin(), elements.end()));

    return result;
}

// Generate all k-combinations
template<typename T>
std::vector<std::vector<T>> generateCombinations(const std::vector<T>& elements, int k) {
    std::vector<std::vector<T>> result;
    int n = elements.size();

    if (k > n) return result;

    std::vector<bool> selector(n, false);
    std::fill(selector.begin(), selector.begin() + k, true);

    do {
        std::vector<T> combination;
        for (int i = 0; i < n; i++) {
            if (selector[i]) {
                combination.push_back(elements[i]);
            }
        }
        result.push_back(combination);
    } while (std::prev_permutation(selector.begin(), selector.end()));

    return result;
}

// Generate all subsets using bitmask
template<typename T>
std::vector<std::vector<T>> generateSubsets(const std::vector<T>& elements) {
    std::vector<std::vector<T>> result;
    int n = elements.size();
    int total = 1 << n;  // 2^n subsets

    for (int mask = 0; mask < total; mask++) {
        std::vector<T> subset;
        for (int i = 0; i < n; i++) {
            if (mask & (1 << i)) {
                subset.push_back(elements[i]);
            }
        }
        result.push_back(subset);
    }
    return result;
}

// Catalan numbers (binary trees, parentheses matching)
unsigned long long catalan(int n) {
    if (n <= 1) return 1;

    std::vector<unsigned long long> dp(n + 1, 0);
    dp[0] = dp[1] = 1;

    for (int i = 2; i <= n; i++) {
        for (int j = 0; j < i; j++) {
            dp[i] += dp[j] * dp[i - 1 - j];
        }
    }
    return dp[n];
}
```

### Practical Applications

1. **Password Generation**: Counting possible combinations
2. **Lottery Systems**: Probability calculations
3. **Scheduling**: Arranging tasks or events
4. **Testing**: Generating test cases
5. **Cryptography**: Key space analysis
6. **Dynamic Programming**: Counting paths, subsequences

---

## Graph Theory

### Mathematical Foundation

A graph G = (V, E) consists of vertices V and edges E:
- **Directed vs Undirected**: Edges have direction or not
- **Weighted vs Unweighted**: Edges have weights or not
- **Connected**: Path exists between any two vertices
- **Cycle**: Path that starts and ends at same vertex
- **DAG**: Directed Acyclic Graph (no cycles)

### C++ Implementation

```cpp
#include <vector>
#include <queue>
#include <stack>
#include <unordered_map>
#include <unordered_set>
#include <limits>
#include <algorithm>

template<typename T>
class Graph {
    std::unordered_map<T, std::vector<std::pair<T, int>>> adjList;  // vertex -> [(neighbor, weight)]
    bool directed;

public:
    Graph(bool isDirected = false) : directed(isDirected) {}

    void addVertex(const T& v) {
        if (adjList.find(v) == adjList.end()) {
            adjList[v] = {};
        }
    }

    void addEdge(const T& from, const T& to, int weight = 1) {
        adjList[from].push_back({to, weight});
        if (!directed) {
            adjList[to].push_back({from, weight});
        }
    }

    // Breadth-First Search
    std::vector<T> bfs(const T& start) {
        std::vector<T> result;
        std::unordered_set<T> visited;
        std::queue<T> q;

        q.push(start);
        visited.insert(start);

        while (!q.empty()) {
            T current = q.front();
            q.pop();
            result.push_back(current);

            for (const auto& [neighbor, weight] : adjList[current]) {
                if (visited.find(neighbor) == visited.end()) {
                    visited.insert(neighbor);
                    q.push(neighbor);
                }
            }
        }
        return result;
    }

    // Depth-First Search
    std::vector<T> dfs(const T& start) {
        std::vector<T> result;
        std::unordered_set<T> visited;
        dfsHelper(start, visited, result);
        return result;
    }

private:
    void dfsHelper(const T& vertex, std::unordered_set<T>& visited, std::vector<T>& result) {
        visited.insert(vertex);
        result.push_back(vertex);

        for (const auto& [neighbor, weight] : adjList[vertex]) {
            if (visited.find(neighbor) == visited.end()) {
                dfsHelper(neighbor, visited, result);
            }
        }
    }

public:
    // Dijkstra's shortest path algorithm
    std::unordered_map<T, int> shortestPaths(const T& start) {
        std::unordered_map<T, int> distances;
        std::unordered_map<T, bool> visited;

        // Initialize distances
        for (const auto& [vertex, _] : adjList) {
            distances[vertex] = std::numeric_limits<int>::max();
        }
        distances[start] = 0;

        // Priority queue: (distance, vertex)
        using P = std::pair<int, T>;
        std::priority_queue<P, std::vector<P>, std::greater<P>> pq;
        pq.push({0, start});

        while (!pq.empty()) {
            auto [dist, current] = pq.top();
            pq.pop();

            if (visited[current]) continue;
            visited[current] = true;

            for (const auto& [neighbor, weight] : adjList[current]) {
                int newDist = dist + weight;
                if (newDist < distances[neighbor]) {
                    distances[neighbor] = newDist;
                    pq.push({newDist, neighbor});
                }
            }
        }
        return distances;
    }

    // Detect cycle using DFS (for directed graphs)
    bool hasCycle() {
        std::unordered_set<T> visited;
        std::unordered_set<T> recStack;

        for (const auto& [vertex, _] : adjList) {
            if (hasCycleUtil(vertex, visited, recStack)) {
                return true;
            }
        }
        return false;
    }

private:
    bool hasCycleUtil(const T& vertex, std::unordered_set<T>& visited,
                      std::unordered_set<T>& recStack) {
        if (recStack.count(vertex)) return true;
        if (visited.count(vertex)) return false;

        visited.insert(vertex);
        recStack.insert(vertex);

        for (const auto& [neighbor, _] : adjList[vertex]) {
            if (hasCycleUtil(neighbor, visited, recStack)) {
                return true;
            }
        }

        recStack.erase(vertex);
        return false;
    }

public:
    // Topological sort (for DAG)
    std::vector<T> topologicalSort() {
        std::unordered_map<T, int> inDegree;
        std::queue<T> q;
        std::vector<T> result;

        // Calculate in-degrees
        for (const auto& [vertex, _] : adjList) {
            inDegree[vertex] = 0;
        }
        for (const auto& [vertex, neighbors] : adjList) {
            for (const auto& [neighbor, _] : neighbors) {
                inDegree[neighbor]++;
            }
        }

        // Add vertices with 0 in-degree
        for (const auto& [vertex, degree] : inDegree) {
            if (degree == 0) {
                q.push(vertex);
            }
        }

        // Process vertices
        while (!q.empty()) {
            T current = q.front();
            q.pop();
            result.push_back(current);

            for (const auto& [neighbor, _] : adjList[current]) {
                inDegree[neighbor]--;
                if (inDegree[neighbor] == 0) {
                    q.push(neighbor);
                }
            }
        }

        return result;
    }
};
```

### Practical Applications

1. **Social Networks**: Friend recommendations, influence analysis
2. **Web Page Ranking**: PageRank algorithm
3. **Route Planning**: GPS navigation (Dijkstra's algorithm)
4. **Dependency Resolution**: Build systems, package managers
5. **Network Flow**: Traffic optimization, matching problems
6. **Compiler Design**: Control flow graphs, data flow analysis
7. **Database**: Query optimization, join algorithms

---

## Number Theory

### Mathematical Foundation

Number theory studies integers and their properties:
- **Prime Numbers**: Numbers divisible only by 1 and themselves
- **GCD/LCM**: Greatest common divisor and least common multiple
- **Modular Arithmetic**: Operations with remainders (a ≡ b (mod m))
- **Euclidean Algorithm**: Efficient GCD computation
- **Fermat's Little Theorem**: a^(p-1) ≡ 1 (mod p) for prime p

### C++ Implementation

```cpp
#include <vector>
#include <cmath>
#include <numeric>

// Greatest Common Divisor (Euclidean algorithm)
int gcd(int a, int b) {
    while (b != 0) {
        int temp = b;
        b = a % b;
        a = temp;
    }
    return a;
}

// Least Common Multiple
int lcm(int a, int b) {
    return (a / gcd(a, b)) * b;  // Avoid overflow
}

// Check if number is prime (basic)
bool isPrime(int n) {
    if (n <= 1) return false;
    if (n <= 3) return true;
    if (n % 2 == 0 || n % 3 == 0) return false;

    for (int i = 5; i * i <= n; i += 6) {
        if (n % i == 0 || n % (i + 2) == 0) {
            return false;
        }
    }
    return true;
}

// Sieve of Eratosthenes: find all primes up to n
std::vector<int> sieveOfEratosthenes(int n) {
    std::vector<bool> isPrime(n + 1, true);
    std::vector<int> primes;

    isPrime[0] = isPrime[1] = false;

    for (int i = 2; i <= n; i++) {
        if (isPrime[i]) {
            primes.push_back(i);
            for (long long j = (long long)i * i; j <= n; j += i) {
                isPrime[j] = false;
            }
        }
    }
    return primes;
}

// Prime factorization
std::vector<std::pair<int, int>> primeFactorize(int n) {
    std::vector<std::pair<int, int>> factors;  // (prime, count)

    // Check for 2
    int count = 0;
    while (n % 2 == 0) {
        count++;
        n /= 2;
    }
    if (count > 0) {
        factors.push_back({2, count});
    }

    // Check odd factors
    for (int i = 3; i * i <= n; i += 2) {
        count = 0;
        while (n % i == 0) {
            count++;
            n /= i;
        }
        if (count > 0) {
            factors.push_back({i, count});
        }
    }

    if (n > 2) {
        factors.push_back({n, 1});
    }

    return factors;
}

// Modular exponentiation: (base^exp) % mod
long long modPow(long long base, long long exp, long long mod) {
    long long result = 1;
    base %= mod;

    while (exp > 0) {
        if (exp % 2 == 1) {
            result = (result * base) % mod;
        }
        base = (base * base) % mod;
        exp /= 2;
    }
    return result;
}

// Extended Euclidean Algorithm
// Returns gcd(a, b) and finds x, y such that ax + by = gcd(a, b)
int extendedGCD(int a, int b, int& x, int& y) {
    if (b == 0) {
        x = 1;
        y = 0;
        return a;
    }

    int x1, y1;
    int gcd = extendedGCD(b, a % b, x1, y1);

    x = y1;
    y = x1 - (a / b) * y1;

    return gcd;
}

// Modular multiplicative inverse
// Returns x such that (a * x) % m = 1
int modInverse(int a, int m) {
    int x, y;
    int g = extendedGCD(a, m, x, y);

    if (g != 1) {
        return -1;  // Inverse doesn't exist
    }

    return (x % m + m) % m;
}
```

### Practical Applications

1. **Cryptography**: RSA encryption, key generation
2. **Hash Functions**: Modular arithmetic for hash tables
3. **Random Number Generation**: Linear congruential generators
4. **Error Detection**: Checksums, CRC
5. **Load Balancing**: Consistent hashing
6. **Competitive Programming**: Many problems involve number theory

---

## Trees and Hierarchical Structures

### Mathematical Foundation

Trees are connected acyclic graphs with special properties:
- **Root**: Top node (in rooted trees)
- **Parent/Child**: Direct connection relationships
- **Leaf**: Node with no children
- **Height**: Longest path from root to leaf
- **Binary Tree**: Each node has at most 2 children
- **Binary Search Tree**: Left < Parent < Right

### C++ Implementation

```cpp
#include <iostream>
#include <queue>
#include <algorithm>

template<typename T>
struct TreeNode {
    T data;
    TreeNode* left;
    TreeNode* right;

    TreeNode(T val) : data(val), left(nullptr), right(nullptr) {}
};

template<typename T>
class BinarySearchTree {
    TreeNode<T>* root;

public:
    BinarySearchTree() : root(nullptr) {}

    // Insert value
    void insert(T value) {
        root = insertHelper(root, value);
    }

private:
    TreeNode<T>* insertHelper(TreeNode<T>* node, T value) {
        if (node == nullptr) {
            return new TreeNode<T>(value);
        }

        if (value < node->data) {
            node->left = insertHelper(node->left, value);
        } else if (value > node->data) {
            node->right = insertHelper(node->right, value);
        }

        return node;
    }

public:
    // Search for value
    bool search(T value) {
        return searchHelper(root, value);
    }

private:
    bool searchHelper(TreeNode<T>* node, T value) {
        if (node == nullptr) return false;
        if (node->data == value) return true;

        if (value < node->data) {
            return searchHelper(node->left, value);
        } else {
            return searchHelper(node->right, value);
        }
    }

public:
    // Inorder traversal (left, root, right) - gives sorted order
    void inorder() {
        inorderHelper(root);
        std::cout << "\n";
    }

private:
    void inorderHelper(TreeNode<T>* node) {
        if (node == nullptr) return;
        inorderHelper(node->left);
        std::cout << node->data << " ";
        inorderHelper(node->right);
    }

public:
    // Preorder traversal (root, left, right)
    void preorder() {
        preorderHelper(root);
        std::cout << "\n";
    }

private:
    void preorderHelper(TreeNode<T>* node) {
        if (node == nullptr) return;
        std::cout << node->data << " ";
        preorderHelper(node->left);
        preorderHelper(node->right);
    }

public:
    // Postorder traversal (left, right, root)
    void postorder() {
        postorderHelper(root);
        std::cout << "\n";
    }

private:
    void postorderHelper(TreeNode<T>* node) {
        if (node == nullptr) return;
        postorderHelper(node->left);
        postorderHelper(node->right);
        std::cout << node->data << " ";
    }

public:
    // Level-order traversal (breadth-first)
    void levelOrder() {
        if (root == nullptr) return;

        std::queue<TreeNode<T>*> q;
        q.push(root);

        while (!q.empty()) {
            TreeNode<T>* current = q.front();
            q.pop();

            std::cout << current->data << " ";

            if (current->left) q.push(current->left);
            if (current->right) q.push(current->right);
        }
        std::cout << "\n";
    }

    // Calculate height
    int height() {
        return heightHelper(root);
    }

private:
    int heightHelper(TreeNode<T>* node) {
        if (node == nullptr) return 0;
        return 1 + std::max(heightHelper(node->left), heightHelper(node->right));
    }

public:
    // Count nodes
    int countNodes() {
        return countNodesHelper(root);
    }

private:
    int countNodesHelper(TreeNode<T>* node) {
        if (node == nullptr) return 0;
        return 1 + countNodesHelper(node->left) + countNodesHelper(node->right);
    }

public:
    // Check if balanced (height difference <= 1)
    bool isBalanced() {
        return isBalancedHelper(root) != -1;
    }

private:
    int isBalancedHelper(TreeNode<T>* node) {
        if (node == nullptr) return 0;

        int leftHeight = isBalancedHelper(node->left);
        if (leftHeight == -1) return -1;

        int rightHeight = isBalancedHelper(node->right);
        if (rightHeight == -1) return -1;

        if (std::abs(leftHeight - rightHeight) > 1) return -1;

        return 1 + std::max(leftHeight, rightHeight);
    }
};
```

### Practical Applications

1. **File Systems**: Directory hierarchies
2. **DOM Trees**: HTML/XML document structure
3. **Expression Parsing**: Abstract syntax trees
4. **Database Indexing**: B-trees, B+ trees
5. **Decision Trees**: Machine learning, game AI
6. **Huffman Coding**: Data compression
7. **Heap**: Priority queues

---

## Recursion and Mathematical Induction

### Mathematical Foundation

**Recursion** and **mathematical induction** are deeply connected:
- **Base Case**: The simplest case (n = 0 or n = 1)
- **Recursive Case**: Express problem in terms of smaller version
- **Inductive Step**: Prove P(n) → P(n+1)

### C++ Implementation

```cpp
#include <vector>
#include <string>

// Fibonacci sequence
int fibonacci(int n) {
    if (n <= 1) return n;  // Base case
    return fibonacci(n - 1) + fibonacci(n - 2);  // Recursive case
}

// Fibonacci with memoization (dynamic programming)
int fibonacciMemo(int n, std::vector<int>& memo) {
    if (n <= 1) return n;
    if (memo[n] != -1) return memo[n];

    memo[n] = fibonacciMemo(n - 1, memo) + fibonacciMemo(n - 2, memo);
    return memo[n];
}

// Tower of Hanoi (classic recursion problem)
void towerOfHanoi(int n, char from, char to, char aux) {
    if (n == 1) {
        std::cout << "Move disk 1 from " << from << " to " << to << "\n";
        return;
    }

    towerOfHanoi(n - 1, from, aux, to);
    std::cout << "Move disk " << n << " from " << from << " to " << to << "\n";
    towerOfHanoi(n - 1, aux, to, from);
}

// Generate all binary strings of length n
void generateBinaryStrings(int n, std::string current = "") {
    if (n == 0) {
        std::cout << current << "\n";
        return;
    }

    generateBinaryStrings(n - 1, current + "0");
    generateBinaryStrings(n - 1, current + "1");
}

// Merge sort (divide and conquer)
void merge(std::vector<int>& arr, int left, int mid, int right) {
    int n1 = mid - left + 1;
    int n2 = right - mid;

    std::vector<int> L(n1), R(n2);

    for (int i = 0; i < n1; i++)
        L[i] = arr[left + i];
    for (int i = 0; i < n2; i++)
        R[i] = arr[mid + 1 + i];

    int i = 0, j = 0, k = left;

    while (i < n1 && j < n2) {
        if (L[i] <= R[j]) {
            arr[k++] = L[i++];
        } else {
            arr[k++] = R[j++];
        }
    }

    while (i < n1) arr[k++] = L[i++];
    while (j < n2) arr[k++] = R[j++];
}

void mergeSort(std::vector<int>& arr, int left, int right) {
    if (left >= right) return;  // Base case

    int mid = left + (right - left) / 2;

    mergeSort(arr, left, mid);      // Recursive case
    mergeSort(arr, mid + 1, right);
    merge(arr, left, mid, right);
}

// Count paths in a grid (n x m) - dynamic programming
int countPaths(int n, int m) {
    if (n == 1 || m == 1) return 1;  // Base case: edge of grid
    return countPaths(n - 1, m) + countPaths(n, m - 1);
}

// Backtracking: N-Queens problem
bool isSafe(std::vector<std::vector<int>>& board, int row, int col, int n) {
    // Check column
    for (int i = 0; i < row; i++) {
        if (board[i][col] == 1) return false;
    }

    // Check upper left diagonal
    for (int i = row, j = col; i >= 0 && j >= 0; i--, j--) {
        if (board[i][j] == 1) return false;
    }

    // Check upper right diagonal
    for (int i = row, j = col; i >= 0 && j < n; i--, j++) {
        if (board[i][j] == 1) return false;
    }

    return true;
}

bool solveNQueens(std::vector<std::vector<int>>& board, int row, int n) {
    if (row >= n) return true;  // Base case: all queens placed

    for (int col = 0; col < n; col++) {
        if (isSafe(board, row, col, n)) {
            board[row][col] = 1;  // Place queen

            if (solveNQueens(board, row + 1, n)) {
                return true;
            }

            board[row][col] = 0;  // Backtrack
        }
    }

    return false;
}
```

### Practical Applications

1. **Divide and Conquer**: Merge sort, quick sort, binary search
2. **Backtracking**: Sudoku solver, maze solving, constraint satisfaction
3. **Dynamic Programming**: Optimization problems
4. **Tree Traversal**: DFS on trees and graphs
5. **Parsing**: Recursive descent parsers
6. **Fractals**: Graphics and procedural generation

---

## Algorithm Analysis and Complexity

### Mathematical Foundation

Big-O notation describes algorithm efficiency:
- **O(1)**: Constant time
- **O(log n)**: Logarithmic (binary search)
- **O(n)**: Linear (array traversal)
- **O(n log n)**: Linearithmic (merge sort)
- **O(n²)**: Quadratic (nested loops)
- **O(2ⁿ)**: Exponential (recursive Fibonacci)
- **O(n!)**: Factorial (permutations)

### Analysis Examples

```cpp
// O(1) - Constant time
int getFirst(const std::vector<int>& arr) {
    return arr[0];  // Single operation
}

// O(log n) - Logarithmic time
int binarySearch(const std::vector<int>& arr, int target) {
    int left = 0, right = arr.size() - 1;

    while (left <= right) {
        int mid = left + (right - left) / 2;

        if (arr[mid] == target) return mid;
        if (arr[mid] < target) left = mid + 1;
        else right = mid - 1;
    }

    return -1;
}

// O(n) - Linear time
int sum(const std::vector<int>& arr) {
    int total = 0;
    for (int num : arr) {  // n iterations
        total += num;
    }
    return total;
}

// O(n log n) - Linearithmic time
// Merge sort divides array (log n levels) and merges (n work per level)
// Already shown above

// O(n²) - Quadratic time
void bubbleSort(std::vector<int>& arr) {
    int n = arr.size();
    for (int i = 0; i < n - 1; i++) {         // n iterations
        for (int j = 0; j < n - i - 1; j++) {  // n iterations
            if (arr[j] > arr[j + 1]) {
                std::swap(arr[j], arr[j + 1]);
            }
        }
    }
}

// O(2ⁿ) - Exponential time
int fibonacci_slow(int n) {
    if (n <= 1) return n;
    return fibonacci_slow(n - 1) + fibonacci_slow(n - 2);  // 2 branches at each level
}

// Improved to O(n) with memoization
int fibonacci_fast(int n) {
    std::vector<int> dp(n + 1);
    dp[0] = 0;
    dp[1] = 1;

    for (int i = 2; i <= n; i++) {
        dp[i] = dp[i - 1] + dp[i - 2];  // O(1) per iteration
    }

    return dp[n];
}
```

### Space Complexity

```cpp
// O(1) space - constant extra space
void reverseArray(std::vector<int>& arr) {
    int left = 0, right = arr.size() - 1;
    while (left < right) {
        std::swap(arr[left++], arr[right--]);
    }
}

// O(n) space - linear extra space
std::vector<int> copyArray(const std::vector<int>& arr) {
    std::vector<int> copy = arr;  // New array of size n
    return copy;
}

// O(log n) space - recursion stack for binary search tree
int treeHeight(TreeNode<int>* node) {
    if (node == nullptr) return 0;
    return 1 + std::max(treeHeight(node->left), treeHeight(node->right));
}
```

### Practical Applications

1. **Algorithm Selection**: Choose right algorithm for data size
2. **Optimization**: Identify bottlenecks
3. **Scalability**: Predict performance with larger datasets
4. **Trade-offs**: Time vs space complexity
5. **Interview Preparation**: Common technical question

---

## Practical Applications

### 1. Data Structure Implementation

Every C++ STL container uses discrete math:

```cpp
#include <unordered_map>
#include <set>
#include <vector>

// Hash table (Set theory + Number theory)
std::unordered_map<std::string, int> hashTable;
hashTable["key"] = 42;  // O(1) average case

// Binary search tree (Tree theory)
std::set<int> bst;
bst.insert(5);  // O(log n)

// Dynamic array (Combinatorics)
std::vector<int> vec;
vec.push_back(10);  // Amortized O(1)
```

### 2. Algorithm Design

```cpp
// Greedy algorithm (optimization)
int coinChange(int amount, const std::vector<int>& coins) {
    int count = 0;
    for (int i = coins.size() - 1; i >= 0 && amount > 0; i--) {
        count += amount / coins[i];
        amount %= coins[i];
    }
    return count;
}

// Dynamic programming (recursion + memoization)
int knapsack(int capacity, const std::vector<int>& weights,
             const std::vector<int>& values) {
    int n = weights.size();
    std::vector<std::vector<int>> dp(n + 1, std::vector<int>(capacity + 1, 0));

    for (int i = 1; i <= n; i++) {
        for (int w = 0; w <= capacity; w++) {
            if (weights[i-1] <= w) {
                dp[i][w] = std::max(dp[i-1][w],
                                   values[i-1] + dp[i-1][w - weights[i-1]]);
            } else {
                dp[i][w] = dp[i-1][w];
            }
        }
    }

    return dp[n][capacity];
}
```

### 3. Real-World Systems

**Networking**: Graph algorithms for routing
```cpp
// Simplified IP routing table using graph
Graph<std::string> network;
network.addEdge("RouterA", "RouterB", 10);  // Weight = latency
auto distances = network.shortestPaths("RouterA");
```

**Databases**: Set operations for queries
```cpp
DiscreteSet<int> users = {1, 2, 3, 4, 5};
DiscreteSet<int> admins = {3, 4};
auto regularUsers = users.difference(admins);  // SQL: EXCEPT
```

**Cryptography**: Number theory
```cpp
// RSA relies on prime factorization difficulty
long long encrypt(long long message, long long e, long long n) {
    return modPow(message, e, n);
}
```

---

## Summary

Discrete mathematics is not just theoretical—it's the foundation of practical programming:

| Concept | C++ Applications |
|---------|------------------|
| **Logic** | Conditionals, bit operations, circuit design |
| **Set Theory** | STL containers, database operations |
| **Functions** | Function pointers, lambdas, functors |
| **Combinatorics** | Permutations, combinations, subset generation |
| **Graph Theory** | Social networks, routing, dependencies |
| **Number Theory** | Cryptography, hashing, random numbers |
| **Trees** | File systems, parsing, search structures |
| **Recursion** | Divide-and-conquer, backtracking, DP |
| **Complexity** | Performance analysis, algorithm selection |

### Key Takeaways

1. **Understanding discrete math makes you a better programmer** by revealing the mathematical structure behind algorithms and data structures.

2. **C++ directly implements discrete math concepts** through templates, STL containers, and algorithms.

3. **Algorithm efficiency** (Big-O) is rooted in discrete math analysis.

4. **Problem-solving techniques** like recursion, dynamic programming, and greedy algorithms all stem from discrete math principles.

5. **Real-world systems** (databases, networks, cryptography) are built on discrete math foundations.

### Further Resources

- **Books**:
  - "Discrete Mathematics and Its Applications" by Kenneth Rosen
  - "Introduction to Algorithms" (CLRS)
  - "The Art of Computer Programming" by Donald Knuth

- **Practice**:
  - LeetCode, HackerRank, Codeforces
  - Project Euler (number theory problems)
  - Graph algorithm visualizations

- **C++ Specific**:
  - cppreference.com for STL documentation
  - Competitive programming contests
  - Open-source C++ projects

---

**Remember**: Discrete mathematics isn't just theory—every time you write C++ code, you're applying these concepts whether you realize it or not. Understanding the math makes you a more effective programmer.
