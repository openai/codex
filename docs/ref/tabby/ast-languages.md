# AST 和语言处理

## 概览

Tabby-Index 使用 **TreeSitter** 提供的增量解析器来提取 AST 和代码标签，支持 **14+ 编程语言**，具备完整的静态分析能力。

## TreeSitter 集成

### 什么是 TreeSitter

**TreeSitter** 是一个用于构建编程语言解析器的库，提供：

- **增量解析**：只重新解析变更的部分，高效处理大文件
- **容错解析**：即使代码有语法错误也能生成有效的 AST
- **语言包**：社区维护的 14+ 编程语言解析器
- **查询 DSL**：.scm (Scheme) 格式的 AST 查询规则

### Tabby-Index 中的集成

```rust
// tree-sitter 依赖结构
Dependency Chain:
  tree-sitter-tags (AST 标签提取)
      ↓
  tree-sitter (core parser)
      ↓
  tree-sitter-{language} (language parsers)
      ├─ tree-sitter-python 0.21.0
      ├─ tree-sitter-rust 0.21.2
      ├─ tree-sitter-java 0.21.0
      ├─ tree-sitter-typescript 0.21.1
      └─ ... (14 languages total)
```

### 代码流程

```rust
// code/intelligence/mod.rs
pub struct CodeIntelligence;

impl CodeIntelligence {
    pub fn find_tags(language: &str, content: &str) -> Result<Vec<Tag>> {
        // 1. 获取语言配置
        let config = LANGUAGE_TAGS.get(language)?;  // lazy_static 缓存

        // 2. 使用 TreeSitter 解析和查询
        let tags = config.generate_tags(content)?;
        //   ├─ 创建 Parser
        //   ├─ 解析代码 → AST
        //   ├─ 应用 .scm 查询规则
        //   └─ 提取匹配的标签

        // 3. 转换为 Tabby 的 Tag 格式
        tags.iter()
            .map(|t| Tag {
                range: (t.start_byte, t.end_byte),
                line_range: (t.start_line as i32, t.end_line as i32),
                is_definition: t.is_definition,
                syntax_type_name: t.syntax_type.to_string(),
                name: t.name.clone(),
                docs: t.docs.clone(),
            })
            .collect()
    }
}
```

## 支持的语言 (14+)

### 语言列表和配置

| # | 语言 | 包名 | 版本 | 源 | 特性 |
|----|------|------|------|-----|------|
| 1 | Python | tree-sitter-python | 0.21.0 | crates.io | 函数、类、装饰器 |
| 2 | Rust | tree-sitter-rust | 0.21.2 | crates.io | 函数、结构体、trait、宏 |
| 3 | Java | tree-sitter-java | 0.21.0 | crates.io | 类、方法、接口、注解 |
| 4 | Kotlin | tree-sitter-kotlin | 0.3.6 | crates.io | 类、函数、扩展 |
| 5 | Scala | tree-sitter-scala | 0.22.1 | crates.io | 类、trait、对象 |
| 6 | TypeScript | tree-sitter-typescript | 0.21.1 | crates.io | 接口、类型、函数、异步 |
| 7 | Go | tree-sitter-go | 0.21.0 | crates.io | 函数、结构体、接口 |
| 8 | Ruby | tree-sitter-ruby | 0.21.0 | crates.io | 类、方法、块 |
| 9 | C | tree-sitter-c | git:00ed08f | GitHub | 函数、结构体、宏 |
| 10 | C++ | tree-sitter-cpp | git:d29fbff | GitHub | 类、模板、命名空间 |
| 11 | C# | tree-sitter-c-sharp | 0.21.2 | crates.io | 类、属性、委托 |
| 12 | Solidity | tree-sitter-solidity | git:custom | GitHub | 合约、函数、事件 |
| 13 | Lua | tree-sitter-lua | 0.1.0 | crates.io | 函数、表 |
| 14 | Elixir | tree-sitter-elixir | 0.2.0 | crates.io | 模块、函数、宏 |
| 15 | GDScript | tree-sitter-gdscript | git:custom | GitHub | 类、函数、信号 |

### 语言分类

#### 系统语言 (编译型)
- **Rust**：完整的函数、结构体、trait、impl 块、宏定义
- **C**：函数、结构体、枚举、宏定义
- **C++**：类、模板、命名空间、成员函数
- **Go**：包级函数、结构体、接口

#### JVM 语言
- **Java**：类、接口、方法、内部类、注解
- **Kotlin**：类、对象、扩展函数、密封类
- **Scala**：类、trait、对象、类型参数

#### 脚本语言
- **Python**：函数、类、装饰器、异步函数
- **Ruby**：类、方法、模块、块
- **Lua**：函数、表、局部函数

#### Web 语言
- **TypeScript**：接口、类型别名、类、异步函数、泛型
- **JavaScript**：通过 TypeScript 解析器支持（TSX）

#### 特殊领域
- **Solidity**：合约、函数、修饰符、事件、状态变量
- **GDScript**：GodotEngine 脚本，类、信号、虚拟函数
- **Elixir**：模块、函数、宏、管道操作

## 标签提取 (Tag Extraction)

### 标签类型 (Syntax Types)

```
常见的标签类型:
├─ function
│  └─ 独立函数、过程
├─ class
│  ├─ 类定义
│  └─ 对象定义 (OOP)
├─ method
│  └─ 类内方法、成员函数
├─ variable
│  ├─ 全局变量
│  └─ 模块级变量
├─ constant
│  └─ 常量定义
├─ struct
│  └─ 结构体、记录类型
├─ enum
│  └─ 枚举类型
├─ interface
│  └─ 接口定义 (Java/TS)
├─ trait
│  └─ trait 定义 (Rust)
├─ module
│  └─ 模块或命名空间
├─ macro
│  └─ 宏定义 (Rust/Elixir)
├─ typedef
│  └─ 类型别名 (TypeScript)
├─ property
│  └─ 属性/字段 (C#/TypeScript)
├─ constructor
│  └─ 构造函数
└─ decorator
   └─ 装饰器/注解 (Python/Java/TS)
```

### Tag 结构

```rust
// code/types.rs
pub struct Tag {
    /// 字符范围 (起始和结束位置，单位: 字节)
    pub range: (usize, usize),

    /// 标签在代码中的行号范围
    pub line_range: (i32, i32),

    /// 是否是定义 (true) 或引用 (false)
    /// Tabby-Index 只索引定义
    pub is_definition: bool,

    /// 语法类型名 (如 "function", "class" 等)
    pub syntax_type_name: String,

    /// 标签名称 (函数名、类名等)
    pub name: String,

    /// 可选的文档字符串
    /// 如 Python docstring、Rust doc comments
    pub docs: Option<String>,
}
```

### 标签提取示例

#### Python 例子

```python
"""Module docstring."""

def process_data(input_file):
    """Process the input file.

    Args:
        input_file: Path to the file
    """
    pass

class DataProcessor:
    """Data processing class."""

    def __init__(self):
        self.data = []

    def process(self, item):
        return item.upper()

@dataclass
class Config:
    name: str
    value: int = 10
```

**提取的标签**:

| name | syntax_type | is_definition | docs |
|------|-------------|--------------|------|
| process_data | function | true | "Process the input file..." |
| DataProcessor | class | true | "Data processing class." |
| __init__ | method | true | - |
| process | method | true | - |
| Config | class | true | - |

#### Rust 例子

```rust
/// Process the data.
pub fn process_data(input: &str) -> Result<String> {
    Ok(input.to_uppercase())
}

/// A data processor struct.
pub struct DataProcessor {
    data: Vec<String>,
}

impl DataProcessor {
    /// Create a new processor.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Process an item.
    pub fn process(&mut self, item: String) {
        self.data.push(item);
    }
}

/// A trait for processable items.
pub trait Processable {
    fn process(&self) -> String;
}
```

**提取的标签**:

| name | syntax_type | is_definition | docs |
|------|-------------|--------------|------|
| process_data | function | true | "Process the data." |
| DataProcessor | struct | true | "A data processor struct." |
| new | method | true | "Create a new processor." |
| process | method | true | "Process an item." |
| Processable | trait | true | "A trait for processable items." |

## 语言配置系统

### 配置存储

```rust
// code/languages.rs (第 185 行)

lazy_static! {
    static ref LANGUAGE_TAGS: HashMap<&'static str, TagsConfigurationSync> = {
        HashMap::from([
            ("python", TagsConfiguration::new(...)),
            ("rust", TagsConfiguration::new(...)),
            ("java", TagsConfiguration::new(...)),
            // ... 14+ languages
        ])
    }
}

// TagsConfigurationSync 是线程安全的包装
pub struct TagsConfigurationSync {
    inner: Arc<TagsConfiguration>,
}

impl TagsConfigurationSync {
    pub fn generate_tags(&self, content: &str) -> Result<Vec<Tag>> {
        // 1. 创建 TreeSitter Parser
        let mut parser = Parser::new();
        parser.set_language(self.language)?;

        // 2. 解析代码
        let tree = parser.parse(content, None)?;

        // 3. 应用 .scm 查询规则
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&self.query, tree.root_node(), content.as_bytes());

        // 4. 提取标签
        let mut tags = vec![];
        for m in matches {
            for capture in m.captures {
                // 处理捕获的节点
                tags.push(Tag { ... });
            }
        }

        Ok(tags)
    }
}
```

### .scm 查询规则

TreeSitter 使用 **Scheme** 格式的查询规则来定义如何提取标签。

#### Python 查询规则示例

```scheme
; 函数定义
(function_definition
  name: (identifier) @name)
  @definition.function

; 类定义
(class_definition
  name: (identifier) @name)
  @definition.class

; 函数内的参数
(parameters
  (identifier) @name)
  @definition.variable

; 异步函数
(async_function_definition
  name: (identifier) @name)
  @definition.function

; 装饰器
(decorator
  name: (identifier) @name)
  @definition.decorator
```

#### Rust 查询规则示例

```scheme
; 函数定义
(function_item
  name: (identifier) @name)
  @definition.function

; 结构体定义
(struct_item
  name: (type_identifier) @name)
  @definition.struct

; trait 定义
(trait_item
  name: (type_identifier) @name)
  @definition.trait

; impl 块中的方法
(impl_item
  (function_item
    name: (identifier) @name))
  @definition.method

; 宏定义
(macro_definition
  name: (identifier) @name)
  @definition.macro
```

**每个语言都有对应的 .scm 文件，定义语言特定的查询规则。**

## 代码指标和有效性检查

### 代码指标

```rust
// code/intelligence/mod.rs 中的 metrics 模块

pub struct CodeMetrics {
    /// 最长行的字符数
    pub max_line_length: i32,

    /// 平均每行字符数
    pub avg_line_length: f32,

    /// 字母和数字的比例 (0.0 - 1.0)
    /// 用于检测非文本文件 (图像、二进制等)
    pub alphanum_fraction: f32,

    /// 数字在内容中的比例
    pub number_fraction: f32,

    /// 代码行数
    pub num_lines: i32,
}

pub fn compute_metrics(content: &str) -> Result<CodeMetrics> {
    let lines: Vec<&str> = content.lines().collect();
    let total_chars: usize = content.len();
    let mut alphanum_count = 0;
    let mut number_count = 0;
    let mut max_length = 0;

    for line in &lines {
        max_length = max_length.max(line.len());
        alphanum_count += line.chars().filter(|c| c.is_alphanumeric()).count();
        number_count += line.chars().filter(|c| c.is_numeric()).count();
    }

    Ok(CodeMetrics {
        max_line_length: max_length as i32,
        avg_line_length: total_chars as f32 / lines.len() as f32,
        alphanum_fraction: alphanum_count as f32 / total_chars as f32,
        number_fraction: number_count as f32 / total_chars as f32,
        num_lines: lines.len() as i32,
    })
}
```

### 有效性检查 (code/index.rs 第 191 行)

```rust
fn is_valid_file(metrics: &CodeMetrics) -> bool {
    metrics.max_line_length <= 300           // 不是超长行文件
        && metrics.avg_line_length <= 150.0   // 平均行长合理
        && metrics.alphanum_fraction >= 0.25  // 足够多的文本内容
        && metrics.num_lines <= 100000        // 不是巨大文件
        && metrics.number_fraction <= 0.50    // 不全是数字 (生成代码/日志)
}
```

**过滤目标**：
- ✗ 生成代码（alphanumeric ratio 低）
- ✗ 极长行（配置文件、编译产物）
- ✗ 二进制文件（转义序列、不可见字符）
- ✗ 构建产物（全是数字、日志）

## 智能代码分块

### 分块策略

```rust
// code/intelligence/mod.rs (第 165 行)

pub fn chunks(content: &str, language: &str) -> Result<Vec<CodeChunk>> {
    // 1. 尝试 CodeSplitter (语义感知)
    match try_code_splitter(language, content) {
        Some(chunks) => {
            // CodeSplitter 按语言特定的结构分块
            // 例如: Rust 按 fn/struct/impl/mod 边界分块
            return Ok(chunks);
        }
        None => {}
    }

    // 2. 降级到 TextSplitter (容错)
    // 如果 CodeSplitter 失败或语言不支持
    let splitter = TextSplitter::new(512);  // 512 字符阈值

    let mut chunks = vec![];
    for chunk_text in splitter.split(content) {
        let chunk = CodeChunk {
            text: chunk_text.to_string(),
            start_line: count_lines_before(content, chunk_text),
            end_line: count_lines_before(content, chunk_text)
                     + chunk_text.lines().count() as i32,
        };
        chunks.push(chunk);
    }

    Ok(chunks)
}

pub struct CodeChunk {
    pub text: String,
    pub start_line: i32,
    pub end_line: i32,
}
```

### 为什么需要两层分块策略？

```
┌─ CodeSplitter (语义感知) ──────────────────┐
│  优点:                                      │
│  • 按代码结构分块 (函数、类等)              │
│  • 保留语义完整性                           │
│  • 避免在关键字中断                         │
│  缺点:                                      │
│  • 只支持部分语言                           │
│  • 初始化开销较大                           │
└─────────────────────────────────────────────┘

┌─ TextSplitter (回退) ──────────────────────┐
│  优点:                                      │
│  • 支持所有语言                             │
│  • 简单高效                                 │
│  缺点:                                      │
│  • 可能在代码中间断开                       │
│  • 损失语义信息                             │
│  优化:                                      │
│  • 512 字符阈值                             │
│  • 尝试在换行符处分割                       │
└─────────────────────────────────────────────┘

混合策略:
  尝试 CodeSplitter
    ↓ 如果成功 → 使用语义感知分块
    ↓ 如果失败 → 降级到 TextSplitter
```

## 源文件 ID 和变更检测

### SourceFileId 机制

```rust
// code/intelligence/id.rs (第 15 行)

pub struct SourceFileId {
    pub path: PathBuf,           // "src/main.rs"
    pub language: String,        // "rust"
    pub git_hash: String,        // SHA256(file_content)
}

impl SourceFileId {
    pub fn compute(path: &str, language: &str, content: &str) -> Result<Self> {
        Ok(SourceFileId {
            path: PathBuf::from(path),
            language: language.to_string(),
            git_hash: sha256::digest(content),
        })
    }

    /// 检查文件是否被修改
    pub fn matches(&self, other: &SourceFileId) -> bool {
        self.path == other.path
            && self.language == other.language
            && self.git_hash == other.git_hash
    }
}
```

### 变更检测流程

```
索引中存储: SourceFileId {path: "src/main.rs", language: "rust", git_hash: "abc123"}
               ↓
               当重新扫描时:
               ↓
当前仓库: 读取 "src/main.rs" → 计算 SHA256 → "abc123"
               ↓
比较结果: git_hash 相同 → 文件未改变 → 跳过索引更新
         git_hash 不同 → 文件已改变 → 重新索引

优势:
  • 无需比较完整内容
  • 支持并发检查 (SHA256 计算独立)
  • 精确检测 (哈希碰撞极罕见)
  • 高效: O(n) 扫描 vs O(n²) 逐行比较
```

## 性能优化

### 缓存策略

| 缓存项 | 存储方式 | 生命周期 | 用途 |
|-------|--------|--------|------|
| **语言配置** | lazy_static | 进程 | 避免重复编译 .scm 规则 |
| **TreeSitter Parser** | Arc<Mutex<>> | 请求 | 复用 parser 对象 |
| **Embedding 缓存** | Redis/Local | 可配置 | 避免重复计算向量 |

### 并发处理

```rust
// 代码索引中的并发步骤

1. File Walk: Sequential (ignore rules)
   ├─ .gitignore 处理必须顺序

2. Per-File Batch: chunks(100)
   ├─ 批处理 100 个文件

3. Per-File Processing: Parallel
   ├─ AST Parsing: Sequential (tree-sitter 线程不安全)
   ├─ Embedding: Parallel (API 并发)
   └─ Indexing: Parallel (Tantivy 线程安全)

4. Commit: Atomic (单线程)
   └─ 确保一致性
```

## 对 codex-rs 的参考建议

### 1. 语言支持

- ✓ 复用 tree-sitter-* 库 (成熟稳定)
- ✓ 可从 Tabby-Index 复制 .scm 查询规则
- ✓ 考虑支持 10+ 主流语言，后续按需扩展

### 2. AST 分析

- ✓ 使用 tree-sitter-tags 提取标签
- ⚠ 需要自定义标签类型 (根据 Rust 特定需求)
- ✓ 实现变更检测 (SourceFileId 模式可复用)

### 3. 代码分块

- ✓ 集成 text-splitter 作为回退
- ⚠ 如有性能要求，可研发 Rust 特定的 CodeSplitter
- ✓ 建议先用 TextSplitter，后优化为 CodeSplitter

### 4. 指标和过滤

- ✓ 复用 CodeMetrics 计算逻辑
- ✓ 复用 is_valid_file 过滤规则
- ⚠ 可根据 Rust 特性调整阈值

---

**相关文档**：
- [核心模块详解](./modules.md) - 实现细节
- [索引构建流程](./indexing-process.md) - 集成方式
