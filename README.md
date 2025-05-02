# Codex-Docs

> 🧠 Fully automated documentation building CLI tool powered by AI

Codex-Docs is an open-source CLI tool that automatically generates comprehensive documentation for your projects using AI. It analyzes your codebase, extracts key information, and builds professional documentation that stays in sync with your code.

Whether you're maintaining a small library or a complex application, Codex-Docs streamlines the documentation process, allowing you to focus on writing code—not docs.

---

## ✨ Features

- ⚡ **Zero-config setup** – Point at your repo and get instant documentation
- 🧠 **Smart code analysis** – Automatically extracts APIs, types, and usage examples
- 📝 **Markdown generation** – Creates clean, well-structured documentation files
- 📄 **README builder** – Constructs detailed README files with essential sections
- 🎨 **Template customization** – Tailor documentation to your project’s branding
- 📦 **Multi-format output** – Export to Markdown, HTML, or PDF
- 🔁 **CI/CD friendly** – Keep docs up-to-date with every commit
- 🤖 **AI-powered summaries** – Generate human-like explanations using OpenAI

---

## 📦 Installation

Install globally using your preferred package manager:

```bash
# npm
npm install -g codex-docs

# yarn
yarn global add codex-docs

# pnpm
pnpm add -g codex-docs
````

---

## 🚀 Quick Start

Generate documentation in seconds:

```bash
# Navigate to your project root
cd your-project

# Generate all documentation
codex-docs generate

# Output to a custom directory
codex-docs generate --output ./docs
```

---

## 💡 Usage Examples

```bash
# Initialize documentation config
codex-docs init

# Generate only the README
codex-docs readme

# Generate only API docs
codex-docs api

# Watch and auto-generate docs on code changes
codex-docs watch

# Use custom templates
codex-docs generate --template custom-template

# Create a full documentation website
codex-docs site --theme modern
```

---

## ⚙️ Configuration

Codex-Docs supports JSON and YAML config files:

```json
{
  "project": {
    "name": "Your Project Name",
    "description": "A short description of your project",
    "version": "1.0.0"
  },
  "output": {
    "dir": "./docs",
    "formats": ["markdown", "html"]
  },
  "templates": {
    "readme": "default",
    "api": "typescript"
  },
  "sections": [
    "installation",
    "usage",
    "api",
    "contributing",
    "license"
  ],
  "exclude": [
    "node_modules/**",
    "dist/**"
  ]
}
```

---

## 🧠 Built With OpenAI

Codex-Docs leverages the power of OpenAI models to:

* Summarize complex code into human-friendly language
* Generate usage examples from your actual APIs
* Highlight edge cases and usage patterns
* Improve documentation clarity and consistency

---

## 🤝 Contributing

We welcome all contributions!

```bash
# Fork the repo
git clone https://github.com/your-username/codex-docs.git
cd codex-docs

# Create a new branch
git checkout -b feature/your-feature-name

# Make changes, commit, and push
git commit -m "Add feature"
git push origin feature/your-feature-name
```

Then open a pull request. Make sure your code follows our style guidelines and passes all tests.

---

## 📜 License

Codex-Docs is released under the [MIT License](./LICENSE).

---

Made with ❤️ by [Khushwant Sanwalot](https://github.com/khushwant04)



---

Would you like me to generate a logo, CLI badge set, or GitHub Actions status shield for this project?

