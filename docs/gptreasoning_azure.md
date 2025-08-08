Azure OpenAI reasoning models are designed to tackle reasoning and problem-solving tasks with increased focus and capability. These models spend more time processing and understanding the user's request, making them exceptionally strong in areas like science, coding, and math compared to previous iterations.

**Key capabilities of reasoning models:**

- Complex Code Generation: Capable of generating algorithms and handling advanced coding tasks to support developers.
- Advanced Problem Solving: Ideal for comprehensive brainstorming sessions and addressing multifaceted challenges.
- Complex Document Comparison: Perfect for analyzing contracts, case files, or legal documents to identify subtle differences.
- Instruction Following and Workflow Management: Particularly effective for managing workflows requiring shorter contexts.

## Availability

### Region availability

| Model        | Region                                                                                                                              | Limited access                                                                                                                                                                                                            |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `gpt-5`      | East US2 & Sweden Central (Global Standard & Data Zones)                                                                            | Request access: [gpt-5 limited access model application](https://aka.ms/oai/gpt5access). If you already have `o3 access` no request is required                                                                           |
| `gpt-5-mini` | East US2 & Sweden Central (Global Standard & Data Zones)                                                                            | No access request needed to use the core capabilities of this model                                                                                                                                                       |
| `gpt-5-nano` | East US2 & Sweden Central (Global Standard & Data Zones)                                                                            | No access request needed to use the core capabilities of this model                                                                                                                                                       |
| `o3-pro`     | East US2 & Sweden Central (Global Standard)                                                                                         | Request access: [o3 limited access model application](https://aka.ms/oai/o3access). If you already have `o3 access` no request is required.                                                                               |
| `codex-mini` | East US2 & Sweden Central (Global Standard)                                                                                         | No access request needed.                                                                                                                                                                                                 |
| `o4-mini`    | [Model availability](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#global-standard-model-availability)  | No access request needed to use the core capabilities of this model.<br><br>Request access: [o4-mini reasoning summary feature](https://aka.ms/oai/o3access)                                                              |
| `o3`         | [Model availability](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#global-standard-model-availability)  | Request access: [o3 limited access model application](https://aka.ms/oai/o3access)                                                                                                                                        |
| `o3-mini`    | [Model availability](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#global-standard-model-availability). | Access is no longer restricted for this model.                                                                                                                                                                            |
| `o1`         | [Model availability](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#global-standard-model-availability). | Access is no longer restricted for this model.                                                                                                                                                                            |
| `o1-mini`    | [Model availability](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#global-standard-model-availability). | No access request needed for Global Standard deployments.<br><br>Standard (regional) deployments are currently only available to select customers who were previously granted access as part of the `o1-preview` release. |

## API & feature support

| **Feature**                                                                                                     | **gpt-5**, **2025-08-07**                                                                                   | **gpt-5-mini**, **2025-08-07**                                                                              | **gpt-5-nano**, **2025-08-07**                                                                              |
| --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **API Version**                                                                                                 | [v1 preview](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/api-version-lifecycle#api-evolution) | [v1 preview](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/api-version-lifecycle#api-evolution) | [v1 preview](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/api-version-lifecycle#api-evolution) |
| **[Developer Messages](#developer-messages)**                                                                   | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| **[Structured Outputs](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/structured-outputs)**   | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| **[Context Window](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/concepts/models#o-series-models)** | Input: 272,000 <br>Output: 128,000                                                                          | Input: 272,000 <br>Output: 128,000                                                                          | Input: 272,000 <br>Output: 128,000                                                                          |
| **[Reasoning effort](#reasoning-effort)**                                                                       | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| **[Image input](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/gpt-with-vision)**             | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| Chat Completions API                                                                                            | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| Responses API                                                                                                   | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| Functions/Tools                                                                                                 | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| Parallel Tool Calls1                                                                                            | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| `max_completion_tokens` 2                                                                                       | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| System Messages 3                                                                                               | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |
| [Reasoning summary](#reasoning-summary) 4                                                                       | ✅                                                                                                          | -                                                                                                           | -                                                                                                           |
| Streaming                                                                                                       | ✅                                                                                                          | ✅                                                                                                          | ✅                                                                                                          |

1 Parallel tool calls are not supported when `reasoning_effort` is set to `minimal`

2 Reasoning models will only work with the `max_completion_tokens` parameter.

3 The latest reasoning models support system messages to make migration easier. You should not use both a developer message and a system message in the same API request.

4 Access to the chain-of-thought reasoning summary is limited access only for `o3` & `o4-mini`.

### NEW GPT-5 reasoning features

| Feature                     | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `reasoning_effort`          | `minimal` is now supported with GPT-5 series reasoning models<br><br>**Options**: `minimal`, `low`, `medium`, `high`                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `verbosity`                 | A new parameter giving you more granular control over how concise the model's output will be.<br><br>**Options:** `low`, `medium`, `high`.                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `preamble`                  | GPT-5 series reasoning models have the ability to spend extra time _"thinking"_ before executing a function/tool call.<br><br>When this planning occurs the model can provide insight into the planning steps in the model response via a new object called the `preamble` object.<br><br>Generation of preambles in the model response is not guaranteed though you can encourage the model by using the `instructions` parameter and passing content like "You MUST plan extensively before each function call. ALWAYS output your plan to the user before calling any function" |
| **allowed tools**           | You can specify multiple tools under `tool_choice` instead of just one.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **custom tool type**        | Enables raw text (non-json) outputs                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| [`lark_tool`](#python-lark) | Allows you to use some of the capabilities of [Python lark](https://github.com/lark-parser/lark) for more flexible constraining of model responses                                                                                                                                                                                                                                                                                                                                                                                                                                 |

Note

- To avoid timeouts [background mode](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/responses#background-tasks) is recommended for `o3-pro`.
- `o3-pro` does not currently support image generation.

### Not Supported

The following are currently unsupported with reasoning models:

- `temperature`, `top_p`, `presence_penalty`, `frequency_penalty`, `logprobs`, `top_logprobs`, `logit_bias`, `max_tokens`

## Usage

These models [don't currently support the same set of parameters](#api--feature-support) as other models that use the chat completions API.

- [Python (Microsoft Entra ID)](#tabpanel_2_python-secure)
- [Python (key-based auth)](#tabpanel_2_python)
- [C#](#tabpanel_2_csharp)

You might need to upgrade your version of the OpenAI Python library to take advantage of the new parameters like `max_completion_tokens`.

```
pip install openai --upgrade
```

```

from openai import AzureOpenAI

client = AzureOpenAI(
  azure_endpoint = os.getenv("AZURE_OPENAI_ENDPOINT"),
  api_key=os.getenv("AZURE_OPENAI_API_KEY"),
  api_version="2025-03-01-preview"
)

response = client.chat.completions.create(
    model="o1-new", # replace with the model deployment name of your o1 deployment.
    messages=[
        {"role": "user", "content": "What steps should I think about when writing my first Python API?"},
    ],
    max_completion_tokens = 5000

)

print(response.model_dump_json(indent=2))
```

### Handling rate limits in Codex

When connecting Codex to Azure reasoning models (e.g., `gpt-5`, `o3`, `o4-mini`), you may occasionally hit Azure throttling and see background messages like "Rate limit is exceeded". Codex automatically retries transient stream errors with exponential backoff and, for rate‑limit errors, enforces a minimum wait before retrying.

You can further tune retries per provider in `~/.codex/config.toml`:

```toml
[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_RESOURCE_NAME.openai.azure.com/openai/v1"
wire_api = "responses"
query_params = { api-version = "preview" }
env_http_headers = { "api-key" = "AZURE_OPENAI_API_KEY" }

# Optional: retry/timeout tuning
request_max_retries = 6
stream_max_retries = 12
stream_idle_timeout_ms = 300000
```

Operational tips:

- Use a region with sufficient capacity for the chosen model.
- Match `model` to the exact Azure deployment name.
- If throttling persists, reduce concurrent requests or adjust Azure quota.

Throughput note

- GPT‑5 soft limit is about 20,000 tokens/second per deployment/region. Size your parallelism so aggregate streamed output stays under that envelope. E.g., 200 tok/s per stream → ~100 concurrent streams. Actual limits vary by quota and region.

**Python Output:**

```
{
  "id": "chatcmpl-AEj7pKFoiTqDPHuxOcirA9KIvf3yz",
  "choices": [
    {
      "finish_reason": "stop",
      "index": 0,
      "logprobs": null,
      "message": {
        "content": "Writing your first Python API is an exciting step in developing software that can communicate with other applications. An API (Application Programming Interface) allows different software systems to interact with each other, enabling data exchange and functionality sharing. Here are the steps you should consider when creating your first Python API...truncated for brevity.",
        "refusal": null,
        "role": "assistant",
        "function_call": null,
        "tool_calls": null
      },
      "content_filter_results": {
        "hate": {
          "filtered": false,
          "severity": "safe"
        },
        "protected_material_code": {
          "filtered": false,
          "detected": false
        },
        "protected_material_text": {
          "filtered": false,
          "detected": false
        },
        "self_harm": {
          "filtered": false,
          "severity": "safe"
        },
        "sexual": {
          "filtered": false,
          "severity": "safe"
        },
        "violence": {
          "filtered": false,
          "severity": "safe"
        }
      }
    }
  ],
  "created": 1728073417,
  "model": "o1-2024-12-17",
  "object": "chat.completion",
  "service_tier": null,
  "system_fingerprint": "fp_503a95a7d8",
  "usage": {
    "completion_tokens": 1843,
    "prompt_tokens": 20,
    "total_tokens": 1863,
    "completion_tokens_details": {
      "audio_tokens": null,
      "reasoning_tokens": 448
    },
    "prompt_tokens_details": {
      "audio_tokens": null,
      "cached_tokens": 0
    }
  },
  "prompt_filter_results": [
    {
      "prompt_index": 0,
      "content_filter_results": {
        "custom_blocklists": {
          "filtered": false
        },
        "hate": {
          "filtered": false,
          "severity": "safe"
        },
        "jailbreak": {
          "filtered": false,
          "detected": false
        },
        "self_harm": {
          "filtered": false,
          "severity": "safe"
        },
        "sexual": {
          "filtered": false,
          "severity": "safe"
        },
        "violence": {
          "filtered": false,
          "severity": "safe"
        }
      }
    }
  ]
}
```

## Reasoning effort

Note

Reasoning models have `reasoning_tokens` as part of `completion_tokens_details` in the model response. These are hidden tokens that aren't returned as part of the message response content but are used by the model to help generate a final answer to your request. `reasoning_effort` can be set to `low`, `medium`, or `high` for all reasoning models except `o1-mini`. GPT-5 reasoning models support a new `reasoning_effort` setting of `minimal`. The higher the effort setting, the longer the model will spend processing the request, which will generally result in a larger number of `reasoning_tokens`.

## Developer messages

Functionally developer messages `"role": "developer"` are the same as system messages.

Adding a developer message to the previous code example would look as follows:

- [Python (Microsoft Entra ID)](#tabpanel_3_python-secure)
- [Python (key-based auth)](#tabpanel_3_python)
- [C#](#tabpanel_3_csharp)

You might need to upgrade your version of the OpenAI Python library to take advantage of the new parameters like `max_completion_tokens`.

```
pip install openai --upgrade
```

```

from openai import AzureOpenAI

client = AzureOpenAI(
  azure_endpoint = os.getenv("AZURE_OPENAI_ENDPOINT"),
  api_key=os.getenv("AZURE_OPENAI_API_KEY"),
  api_version="2025-04-01-preview"
)

response = client.chat.completions.create(
    model="o1-new", # replace with the model deployment name of your o1 deployment.
    messages=[
        {"role": "developer","content": "You are a helpful assistant."}, # optional equivalent to a system message for reasoning models
        {"role": "user", "content": "What steps should I think about when writing my first Python API?"},
    ],
    max_completion_tokens = 5000,
    reasoning_effort = "medium" # low, medium, or high
)

print(response.model_dump_json(indent=2))
```

## Reasoning summary

When using the latest reasoning models with the [Responses API](https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/responses) you can use the reasoning summary parameter to receive summaries of the model's chain of thought reasoning. This parameter can be set to `auto`, `concise`, or `detailed`. Access to this feature requires you to [Request Access](https://aka.ms/oai/o3access).

Note

Even when enabled, reasoning summaries are not generated for every step/request. This is expected behavior.

- [Python](#tabpanel_4_py)
- [REST](#tabpanel_4_REST)

You'll need to upgrade your OpenAI client library for access to the latest parameters.

```
pip install openai --upgrade
```

```
from openai import AzureOpenAI
from azure.identity import DefaultAzureCredential, get_bearer_token_provider

token_provider = get_bearer_token_provider(
    DefaultAzureCredential(), "https://cognitiveservices.azure.com/.default"
)

client = AzureOpenAI(
  base_url = "https://YOUR-RESOURCE-NAME.openai.azure.com/openai/v1/",
  azure_ad_token_provider=token_provider,
  api_version="preview"
)

response = client.responses.create(
    input="Tell me about the curious case of neural text degeneration",
    model="o4-mini", # replace with model deployment name
    reasoning={
        "effort": "medium",
        "summary": "detailed" # auto, concise, or detailed (currently only supported with o4-mini and o3)
    }
)

print(response.model_dump_json(indent=2))
```

```
{
  "id": "resp_68007e26b2cc8190b83361014f3a78c50ae9b88522c3ad24",
  "created_at": 1744862758.0,
  "error": null,
  "incomplete_details": null,
  "instructions": null,
  "metadata": {},
  "model": "o4-mini",
  "object": "response",
  "output": [
    {
      "id": "rs_68007e2773bc8190b5b8089949bfe13a0ae9b88522c3ad24",
      "summary": [
        {
          "text": "**Summarizing neural text degeneration**\n\nThe user's asking about \"The Curious Case of Neural Text Degeneration,\" a paper by Ari Holtzman et al. from 2020. It explains how certain decoding strategies produce repetitive and dull text. In contrast, methods like nucleus sampling yield more coherent and diverse outputs. The authors introduce metrics like surprisal and distinct-n for evaluation and suggest that maximum likelihood decoding often favors generic continuations, leading to loops and repetitive patterns in longer texts. They promote sampling from truncated distributions for improved text quality.",
          "type": "summary_text"
        },
        {
          "text": "**Explaining nucleus sampling**\n\nThe authors propose nucleus sampling, which captures a specified mass of the predictive distribution, improving metrics such as coherence and diversity. They identify a \"sudden drop\" phenomenon in token probabilities, where a few tokens dominate, leading to a long tail. By truncating this at a cumulative probability threshold, they aim to enhance text quality compared to top-k sampling. Their evaluations include human assessments, showing better results in terms of BLEU scores and distinct-n measures. Overall, they highlight how decoding strategies influence quality and recommend adaptive techniques for improved outcomes.",
          "type": "summary_text"
        }
      ],
      "type": "reasoning",
      "status": null
    },
    {
      "id": "msg_68007e35c44881908cb4651b8e9972300ae9b88522c3ad24",
      "content": [
        {
          "annotations": [],
          "text": "Researchers first became aware that neural language models, when used to generate long stretches of text with standard “maximum‐likelihood” decoding (greedy search, beam search, etc.), often produce bland, repetitive or looping output. The 2020 paper “The Curious Case of Neural Text Degeneration” (Holtzman et al.) analyzes this failure mode and proposes a simple fix—nucleus (top‑p) sampling—that dramatically improves output quality.\n\n1. The Problem: Degeneration  \n   • With greedy or beam search, models tend to pick very high‑probability tokens over and over, leading to loops (“the the the…”) or generic, dull continuations.  \n   • Even sampling with a fixed top‑k (e.g. always sample from the 40 most likely tokens) can be suboptimal: if the model’s probability mass is skewed, k may be too small (overly repetitive) or too large (introducing incoherence).\n\n2. Why It Happens: Distributional Peakedness  \n   • At each time step the model’s predicted next‐token distribution often has one or two very high‑probability tokens, then a long tail of low‑probability tokens.  \n   • Maximum‐likelihood decoding zeroes in on the peak, collapsing diversity.  \n   • Uniform sampling over a large k allows low‑probability “wild” tokens, harming coherence.\n\n3. The Fix: Nucleus (Top‑p) Sampling  \n   • Rather than fixing k, dynamically truncate the distribution to the smallest set of tokens whose cumulative probability ≥ p (e.g. p=0.9).  \n   • Then renormalize and sample from that “nucleus.”  \n   • This keeps only the “plausible” mass and discards the improbable tail, adapting to each context.\n\n4. Empirical Findings  \n   • Automatic metrics (distinct‑n, repetition rates) and human evaluations show nucleus sampling yields more diverse, coherent, on‑topic text than greedy/beam or fixed top‑k.  \n   • It also outperforms simple temperature scaling (raising logits to 1/T) because it adapts to changes in the distribution’s shape.\n\n5. Takeaways for Practitioners  \n   • Don’t default to beam search for open-ended generation—its high likelihood doesn’t mean high quality.  \n   • Use nucleus sampling (p between 0.8 and 0.95) for a balance of diversity and coherence.  \n   • Monitor repetition and distinct‑n scores if you need automatic sanity checks.\n\nIn short, “neural text degeneration” is the tendency of likelihood‐maximizing decoders to produce dull or looping text. By recognizing that the shape of the model’s probability distribution varies wildly from step to step, nucleus sampling provides an elegant, adaptive way to maintain both coherence and diversity in generated text.",
          "type": "output_text"
        }
      ],
      "role": "assistant",
      "status": "completed",
      "type": "message"
    }
  ],
  "parallel_tool_calls": true,
  "temperature": 1.0,
  "tool_choice": "auto",
  "tools": [],
  "top_p": 1.0,
  "max_output_tokens": null,
  "previous_response_id": null,
  "reasoning": {
    "effort": "medium",
    "generate_summary": null,
    "summary": "detailed"
  },
  "status": "completed",
  "text": {
    "format": {
      "type": "text"
    }
  },
  "truncation": "disabled",
  "usage": {
    "input_tokens": 16,
    "output_tokens": 974,
    "output_tokens_details": {
      "reasoning_tokens": 384
    },
    "total_tokens": 990,
    "input_tokens_details": {
      "cached_tokens": 0
    }
  },
  "user": null,
  "store": true
}
```

## Python lark

GPT-5 series reasoning models have the ability to call a new `custom_tool` called `lark_tool`. This tool is based on [Python lark](https://github.com/lark-parser/lark) and can be used for more flexible constraining of model output.

### Chat Completions

```
{
  "messages": [
    {
      "role": "user",
      "content": "Which one is larger, 42 or 0?"
    }
  ],
  "tools": [
    {
      "type": "custom",
      "name": "custom_tool",
      "custom": {
        "name": "lark_tool",
        "format": {
          "type": "grammar",
          "grammar": {
            "syntax": "lark",
            "definition": "start: QUESTION NEWLINE ANSWER\nQUESTION: /[^\\n?]{1,200}\\?/\nNEWLINE: /\\n/\nANSWER: /[^\\n!]{1,200}!/"
          }
        }
      }
    }
  ],
  "tool_choice": "required",
  "model": "gpt-5-2025-08-07"
}
```

### Responses API

```
{
  "model": "gpt-5-2025-08-07",
  "input": "please calculate the area of a circle with radius equal to the number of 'r's in strawberry",
  "tools": [
    {
      "type": "custom",
      "name": "lark_tool",
      "format": {
        "type": "grammar",
        "syntax": "lark",
        "definition": "start: QUESTION NEWLINE ANSWER\nQUESTION: /[^\\n?]{1,200}\\?/\nNEWLINE: /\\n/\nANSWER: /[^\\n!]{1,200}!/"
      }
    }
  ],
  "tool_choice": "required"
}
```

## Markdown output

By default the `o3-mini` and `o1` models will not attempt to produce output that includes markdown formatting. A common use case where this behavior is undesirable is when you want the model to output code contained within a markdown code block. When the model generates output without markdown formatting you lose features like syntax highlighting, and copyable code blocks in interactive playground experiences. To override this new default behavior and encourage markdown inclusion in model responses, add the string `Formatting re-enabled` to the beginning of your developer message.

Adding `Formatting re-enabled` to the beginning of your developer message does not guarantee that the model will include markdown formatting in its response, it only increases the likelihood. We have found from internal testing that `Formatting re-enabled` is less effective by itself with the `o1` model than with `o3-mini`.

To improve the performance of `Formatting re-enabled` you can further augment the beginning of the developer message which will often result in the desired output. Rather than just adding `Formatting re-enabled` to the beginning of your developer message, you can experiment with adding a more descriptive initial instruction like one of the examples below:

- `Formatting re-enabled - please enclose code blocks with appropriate markdown tags.`
- `Formatting re-enabled - code output should be wrapped in markdown.`

Depending on your expected output you may need to customize your initial developer message further to target your specific use case.
