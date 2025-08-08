# GPT-5 New Parameters and Tools

We’re introducing new developer controls in the GPT-5 series that give you greater control over model responses—from shaping output length and style to enforcing strict formatting. Below is a quick overview of the latest features:

## Feature Overview

| #   | Feature                        | Overview                                                                                                                                                                                                                                                                                        | Values / Usage                                                                                                                                         |
| --- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1.  | **Verbosity Parameter**        | Lets you hint the model to be more or less expansive in its replies. Keep prompts stable and use the parameter instead of re-writing.                                                                                                                                                           | • **low** → terse UX, minimal prose.<br>• **medium** _(default)_ → balanced detail.<br>• **high** → verbose, great for audits, teaching, or hand-offs. |
| 2.  | **Free-Form Function Calling** | Generate raw text payloads—anything from Python scripts to SQL queries—directly to your custom tool without JSON wrapping. Offers greater flexibility for external runtimes like:<br>• Code sandboxes (Python, C++, Java, …)<br>• SQL databases<br>• Shell environments<br>• Config generators  | Use when structured JSON isn’t needed and raw text is more natural for the target tool.                                                                |
| 3.  | **Context-Free Grammar (CFG)** | A set of production rules defining valid strings in a language. Each rule rewrites a non-terminal into terminals and/or other non-terminals, independent of surrounding context. Useful for constraining output to match the syntax of programming languages or custom formats in OpenAI tools. | Use as a contract to ensure the model emits only valid strings accepted by the grammar.                                                                |

**Supported Models:**

- gpt-5
- gpt-5-mini
- gpt-5-nano

**Supported API Endpoints**

- Responses API
- Chat Completions API

**Note:** We recommend using the Responses API with GPT-5 series models to get the most performance out of the models.

## Pre-requisites

To begin, update your OpenAI SDK to support the new parameters and tools for GPT-5. Ensure you've set the `OPENAI_API_KEY` as an environment variable.

```bash
pip install --quiet --upgrade openai pandas
```

## 1. Verbosity Parameter

### 1.1 Overview

The verbosity parameter lets you hint the model to be more or less expansive in its replies.

**Values:** "low", "medium", "high"

- **low** → terse UX, minimal prose.
- **medium** (default) → balanced detail.
- **high** → verbose, great for audits, teaching, or hand-offs.

Keep prompts stable and use the parameter instead of re-writing.

### 1.2 Example: Generating a Poem

```python
from openai import OpenAI
import pandas as pd
from IPython.display import display

client = OpenAI()

question = "Write a poem about a boy and his first pet dog."

data = []

for verbosity in ["low", "medium", "high"]:
    response = client.responses.create(
        model="gpt-5-mini",
        input=question,
        text={"verbosity": verbosity}
    )

    output_text = ""
    for item in response.output:
        if hasattr(item, "content"):
            for content in item.content:
                if hasattr(content, "text"):
                    output_text += content.text

    if len(output_text) > 700:
        sample_output = output_text[:500] + " ... redacted for brevity ... " + output_text[-200:]
    else:
        sample_output = output_text

    usage = response.usage
    data.append({
        "Verbosity": verbosity,
        "Sample Output": sample_output,
        "Output Tokens": usage.output_tokens
    })

df = pd.DataFrame(data)

styled_df = df.style.set_table_styles([
    {'selector': 'th', 'props': [('text-align', 'center')]},
    {'selector': 'td', 'props': [('text-align', 'left')]}
])

display(styled_df)
```

The output tokens scale roughly linearly with verbosity: low (754) → medium (939) → high (1174).

### 1.3 Using Verbosity for Coding Use Cases

The verbosity parameter also influences the length and complexity of generated code, as well as the depth of accompanying explanations. Here's an example of generating a Python program to sort an array of 1,000,000 random numbers.

```python
from openai import OpenAI

client = OpenAI()

prompt = "Output a Python program that sorts an array of 1000000 random numbers"

def ask_with_verbosity(verbosity: str, question: str):
    response = client.responses.create(
        model="gpt-5-mini",
        input=question,
        text={"verbosity": verbosity}
    )

    output_text = ""
    for item in response.output:
        if hasattr(item, "content"):
            for content in item.content:
                if hasattr(content, "text"):
                    output_text += content.text

    usage = response.usage

    print("--------------------------------")
    print(f"Verbosity: {verbosity}")
    print("Output:")
    print(output_text)
    print(f"Tokens => input: {usage.input_tokens} | output: {usage.output_tokens}")

ask_with_verbosity("low", prompt)
ask_with_verbosity("medium", prompt)
ask_with_verbosity("high", prompt)
```

**Takeaways:**

- **Low verbosity** produces a minimal, functional script with no extra comments or structure.
- **Medium verbosity** adds explanatory comments, function structure, and reproducibility controls.
- **High verbosity** yields a comprehensive, production-ready script with argument parsing, multiple sorting methods, timing/verification, usage notes, and best-practice tips.

## 2. Free-Form Function Calling

### 2.1 Overview

GPT-5 can now send raw text payloads—such as Python scripts, SQL queries, or config files—directly to custom tools without JSON wrapping using the new tool `"type": "custom"`. This provides greater flexibility when interacting with external runtimes.

**Note:** Custom tool type does NOT support parallel tool calling.

### 2.2 Quick Start Example: Compute the Area of a Circle

```python
from openai import OpenAI

client = OpenAI()

response = client.responses.create(
    model="gpt-5-mini",
    input="Please use the code_exec tool to calculate the area of a circle with radius equal to the number of 'r's in strawberry",
    text={"format": {"type": "text"}},
    tools=[
        {
            "type": "custom",
            "name": "code_exec",
            "description": "Executes arbitrary python code",
        }
    ]
)
print(response.output)
```

### 2.3 Mini-Benchmark: Sorting an Array in Three Languages

```python
from openai import OpenAI
from typing import List, Optional

MODEL_NAME = "gpt-5"

TOOLS = [
    {"type": "custom", "name": "code_exec_python", "description": "Executes python code"},
    {"type": "custom", "name": "code_exec_cpp", "description": "Executes c++ code"},
    {"type": "custom", "name": "code_exec_java", "description": "Executes java code"},
]

client = OpenAI()

def create_response(input_messages: List[dict], previous_response_id: Optional[str] = None):
    kwargs = {
        "model": MODEL_NAME,
        "input": input_messages,
        "text": {"format": {"type": "text"}},
        "tools": TOOLS,
    }
    if previous_response_id:
        kwargs["previous_response_id"] = previous_response_id
    return client.responses.create(**kwargs)

def run_conversation(input_messages: List[dict], previous_response_id: Optional[str] = None):
    response = create_response(input_messages, previous_response_id)
    tool_call = response.output[1] if len(response.output) > 1 else None

    if tool_call and tool_call.type == "custom_tool_call":
        print("--- tool name ---")
        print(tool_call.name)
        print("--- tool call argument (generated code) ---")
        print(tool_call.input)

        input_messages.append({
            "type": "function_call_output",
            "call_id": tool_call.call_id,
            "output": "done",
        })

        return run_conversation(input_messages, previous_response_id=response.id)
    else:
        return

prompt = """
Write code to sort the array of numbers in three languages: C++, Python and Java (10 times each)using code_exec functions.
ALWAYS CALL THESE THREE FUNCTIONS EXACTLY ONCE: code_exec_python, code_exec_cpp and code_exec_java tools to sort the array in each language. Stop once you've called these three functions in each language once.
Print only the time it takes to sort the array in milliseconds.
[448, 986, 255, 884, 632, 623, 246, 439, 936, 925, 644, 159, 777, 986, 706, 723, 534, 862, 195, 686, 846, 880, 970, 276, 613, 736, 329, 622, 870, 284, 945, 708, 267, 327, 678, 807, 687, 890, 907, 645, 364, 333, 385, 262, 730, 603, 945, 358, 923, 930, 761, 504, 870, 561, 517, 928, 994, 949, 233, 137, 670, 555, 149, 870, 997, 809, 180, 498, 914, 508, 411, 378, 394, 368, 766, 486, 757, 319, 338, 159, 585, 934, 654, 194, 542, 188, 934, 163, 889, 736, 792, 737, 667, 772, 198, 971, 459, 402, 989, 949]
"""

messages = [{"role": "developer", "content": prompt}]
run_conversation(messages)
```

**Takeaways:**

Free-form tool calling in GPT-5 lets you send raw text payloads—such as Python scripts, SQL queries, or config files—directly to custom tools without JSON wrapping. This provides greater flexibility for interacting with external runtimes and allows the model to generate code or text in the exact format your tool expects. It’s ideal when structured JSON is unnecessary and natural text output improves usability.

## 3. Context-Free Grammar (CFG)

### 3.1 Overview

A context-free grammar is a collection of production rules that define which strings belong to a language. Each rule rewrites a non-terminal symbol into a sequence of terminals and/or other non-terminals, independent of surrounding context. CFGs can capture the syntax of most programming languages and, in OpenAI custom tools, serve as contracts that force the model to emit only strings that the grammar accepts.

### 3.2 Grammar Fundamentals

**Supported Grammar Syntax**

- Lark: [https://lark-parser.readthedocs.io/en/stable/](https://lark-parser.readthedocs.io/en/stable/)
- Regex: [https://docs.rs/regex/latest/regex/#syntax](https://docs.rs/regex/latest/regex/#syntax)

**Unsupported Lark Features**

- Lookaround in regexes (`(?=...)`, `(?!...)`, etc.)
- Lazy modifier (`*?`, `+?`, `??`) in regexes.
- Terminal priorities, templates, `%declares`, `%import` (except `%import common`).

**Terminals vs Rules & Greedy Lexing**

| Concept           | Take-away                                                                       |
| ----------------- | ------------------------------------------------------------------------------- |
| Terminals (UPPER) | Matched first by the lexer – longest match wins.                                |
| Rules (lower)     | Combine terminals; cannot influence how text is tokenised.                      |
| Greedy lexer      | Never try to “shape” free text across multiple terminals – you’ll lose control. |

**Correct vs Incorrect Pattern Design**

✅ **One bounded terminal handles free-text between anchors**

```
start: SENTENCE
SENTENCE: /[A-Za-z, ]*(the hero|a dragon)[A-Za-z, ]*(fought|saved)[A-Za-z, ]*(a treasure|the kingdom)[A-Za-z, ]*\./
```

❌ **Don’t split free-text across multiple terminals/rules**

```
start: sentence
sentence: /[A-Za-z, ]+/ subject /[A-Za-z, ]+/ verb /[A-Za-z, ]+/ object /[A-Za-z, ]+/
```

### 3.3 Example: SQL Dialect — MS SQL vs PostgreSQL

```python
import textwrap

mssql_grammar = textwrap.dedent(r"""
    // ---------- Punctuation & operators ----------
    SP: " "
    COMMA: ","
    GT: ">"
    EQ: "="
    SEMI: ";"

    // ---------- Start ----------
    start: "SELECT" SP "TOP" SP NUMBER SP select_list SP "FROM" SP table SP "WHERE" SP amount_filter SP "AND" SP date_filter SP "ORDER" SP "BY" SP sort_cols SEMI

    // ---------- Projections ----------
    select_list: column (COMMA SP column)*
    column: IDENTIFIER

    // ---------- Tables ----------
    table: IDENTIFIER

    // ---------- Filters ----------
    amount_filter: "total_amount" SP GT SP NUMBER
    date_filter: "order_date" SP GT SP DATE

    // ---------- Sorting ----------
    sort_cols: "order_date" SP "DESC"

    // ---------- Terminals ----------
    IDENTIFIER: /[A-Za-z_][A-Za-z0-9_]*/
    NUMBER: /[0-9]+/
    DATE: /'[0-9]{4}-[0-9]{2}-[0-9]{2}'/
""")

postgres_grammar = textwrap.dedent(r"""
    // ---------- Punctuation & operators ----------
    SP: " "
    COMMA: ","
    GT: ">"
    EQ: "="
    SEMI: ";"

    // ---------- Start ----------
    start: "SELECT" SP select_list SP "FROM" SP table SP "WHERE" SP amount_filter SP "AND" SP date_filter SP "ORDER" SP "BY" SP sort_cols SP "LIMIT" SP NUMBER SEMI

    // ---------- Projections ----------
    select_list: column (COMMA SP column)*
    column: IDENTIFIER

    // ---------- Tables ----------
    table: IDENTIFIER

    // ---------- Filters ----------
    amount_filter: "total_amount" SP GT SP NUMBER
    date_filter: "order_date" SP GT SP DATE

    // ---------- Sorting ----------
    sort_cols: "order_date" SP "DESC"

    // ---------- Terminals ----------
    IDENTIFIER: /[A-Za-z_][A-Za-z0-9_]*/
    NUMBER: /[0-9]+/
    DATE: /'[0-9]{4}-[0-9]{2}-[0-9]{2}'/
""")
```

### 3.4 Generate Specific SQL Dialect

```python
from openai import OpenAI
client = OpenAI()

sql_prompt_mssql = (
    "Call the mssql_grammar to generate a query for Microsoft SQL Server that retrieves the "
    "five most recent orders per customer, showing customer_id, order_id, order_date, and total_amount, "
    "where total_amount > 500 and order_date is after '2025-01-01'. "
)

response_mssql = client.responses.create(
    model="gpt-5",
    input=sql_prompt_mssql,
    text={"format": {"type": "text"}},
    tools=[
        {
            "type": "custom",
            "name": "mssql_grammar",
            "description": "Executes read-only Microsoft SQL Server queries limited to SELECT statements with TOP and basic WHERE/ORDER BY. YOU MUST REASON HEAVILY ABOUT THE QUERY AND MAKE SURE IT OBEYS THE GRAMMAR.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": mssql_grammar
            }
        },
    ],
    parallel_tool_calls=False
)

print("--- MS SQL Query ---")
print(response_mssql.output[1].input)

sql_prompt_pg = (
    "Call the postgres_grammar to generate a query for PostgreSQL that retrieves the "
    "five most recent orders per customer, showing customer_id, order_id, order_date, and total_amount, "
    "where total_amount > 500 and order_date is after '2025-01-01'. "
)

response_pg = client.responses.create(
    model="gpt-5",
    input=sql_prompt_pg,
    text={"format": {"type": "text"}},
    tools=[
        {
            "type": "custom",
            "name": "postgres_grammar",
            "description": "Executes read-only PostgreSQL queries limited to SELECT statements with LIMIT and basic WHERE/ORDER BY. YOU MUST REASON HEAVILY ABOUT THE QUERY AND MAKE SURE IT OBEYS THE GRAMMAR.",
            "format": {
                "type": "grammar",
                "syntax": "lark",
                "definition": postgres_grammar
            }
        },
    ],
    parallel_tool_calls=False,
)

print("--- PG SQL Query ---")
print(response_pg.output[1].input)
```

**Output Highlights:**

| Dialect       | Generated Query                                           | Key Difference                          |
| ------------- | --------------------------------------------------------- | --------------------------------------- |
| MS SQL Server | `SELECT TOP 5 customer_id, … ORDER BY order_date DESC;`   | Uses `TOP N` clause before column list. |
| PostgreSQL    | `SELECT customer_id, … ORDER BY order_date DESC LIMIT 5;` | Uses `LIMIT N` after `ORDER BY`.        |

### 3.5 Example: Regex CFG Syntax

```python
from openai import OpenAI
client = OpenAI()

timestamp_grammar_definition = r"^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01]) (?:[01]\d|2[0-3]):[0-5]\d$"

timestamp_prompt = "Call the timestamp_grammar to save a timestamp for August 7th 2025 at 10AM."

response_mssql = client.responses.create(
    model="gpt-5",
    input=timestamp_prompt,
    text={"format": {"type": "text"}},
    tools=[
        {
            "type": "custom",
            "name": "timestamp_grammar",
            "description": "Saves a timestamp in date + time in 24-hr format.",
            "format": {
                "type": "grammar",
                "syntax": "regex",
                "definition": timestamp_grammar_definition
            }
        },
    ],
    parallel_tool_calls=False
)

print("--- Timestamp ---")
print(response_mssql.output[1].input)
```

### 3.5 Best Practices

- **Keep terminals bounded**: Use `/[^.\n]{0,10}*\./` instead of `/.*\./`. Limit matches by content and length.
- **Prefer explicit char-classes** over `.` wildcards.
- **Thread whitespace explicitly**: Use `SP = " "` instead of global `%ignore`.
- **Describe your tool**: Tell the model exactly what the CFG accepts and instruct it to reason heavily about compliance.

**Troubleshooting:**

- **API rejects grammar**: Simplify rules and terminals, remove `%ignore.*`.
- **Unexpected tokens**: Confirm terminals aren’t overlapping; check greedy lexer.
- **Model drifts "out-of-distribution"**: Tighten grammar, iterate on prompt and tool description, experiment with higher reasoning effort.

**Resources:**

- Lark Docs: [https://lark-parser.readthedocs.io/en/stable/](https://lark-parser.readthedocs.io/en/stable/)
- Lark IDE: [https://www.lark-parser.org/ide/](https://www.lark-parser.org/ide/)
- LLGuidance Syntax: [https://github.com/guidance-ai/llguidance/blob/main/docs/syntax.md](https://github.com/guidance-ai/llguidance/blob/main/docs/syntax.md)
- Regex: [https://docs.rs/regex/latest/regex/#syntax](https://docs.rs/regex/latest/regex/#syntax)

### 3.6 Takeaways

Context-Free Grammar (CFG) support in GPT-5 lets you strictly constrain model output to match predefined syntax, ensuring only valid strings are generated. This is especially useful for enforcing programming language rules or custom formats, reducing post-processing and errors. By providing a precise grammar and clear tool description, you can make the model reliably stay within your target output structure.
