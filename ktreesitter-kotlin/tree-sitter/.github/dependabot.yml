version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
    commit-message:
      prefix: "build(deps)"
    labels:
      - "dependencies"
      - "cargo"
    groups:
      cargo:
        patterns: ["*"]
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    commit-message:
      prefix: "ci"
    labels:
      - "dependencies"
      - "github-actions"
    groups:
      actions:
        patterns: ["*"]
