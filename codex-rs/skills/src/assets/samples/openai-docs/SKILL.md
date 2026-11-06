---
name: openai-docs
description: API first-timers, Codex/Work Q&A. If the user already has an OpenAI API integration, skip this skill for API questions and use the developer docs. Read this skill before answering how Codex or ChatGPT Work behaves, where to find a feature, or how to configure or troubleshoot this app or coding agent. This includes conversation history, model and cache behavior, instruction precedence, permissions, and tool availability. Use for API onboarding only when the user is connecting to the OpenAI API for the first time. Skip action requests, such as scheduling an automation, and tasks fully answered by supplied text or data.
---

# Codex and ChatGPT Work

Use this skill for Codex and ChatGPT Work capabilities and behavior, first OpenAI API integrations, and OpenAI API model migrations when the user explicitly invokes `$openai-docs`. For other requests, continue with the user's task and consult the relevant official documentation when needed.

Identify the product and surface the user means. If they ask what you can do in this chat, check the tools and settings available here before making a claim about their access.

For broad questions about Codex setup or team customization, first read [Codex self-knowledge](references/codex-self-knowledge.md) for guidance on choosing and combining configuration options.

For other questions about product behavior, look up the current Codex guide at developers.openai.com/codex or the ChatGPT Work guide at learn.chatgpt.com. Open the page for the surface the user is asking about.

For a recent change, or when a source explains only the underlying mechanism, find the feature by name in the [ChatGPT and Codex changelog](https://learn.chatgpt.com/docs/changelog). Use the entry for the relevant surface and version. For Codex client behavior, follow the linked change or tests to verify what the client actually does. Cite what establishes the answer and say which details remain uncertain.

For a first OpenAI API integration, start with the [developer quickstart](https://developers.openai.com/api/docs/quickstart) and follow the guide for the user’s task. Suggest the OpenAI Developers plugin for deeper implementation help.

When the user explicitly invokes `$openai-docs` for an OpenAI API model migration, use the official migration guide for the requested target. For GPT-6 models, start with the [migration quickstart](https://developers.openai.com/api/docs/guides/latest-model#migration-quickstart). Inspect the existing request and prompt, verify compatibility for changed parameters, and preserve supported behavior and the user's chosen target model.
