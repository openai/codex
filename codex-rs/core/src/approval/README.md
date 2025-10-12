# Command Approval Module

This module is responsible for analyzing shell commands and making a
decision on whether they should be executed. It determines if a command should be
auto-approved, rejected, or if it requires interactive user approval.

## Design Philosophy

The core design philosophy is a **hybrid, multi-layered approach to command analysis**.
This allows us to apply the right level of analytical rigor to each command,
balancing the need for precision and correctness against implementation and maintenance
effort.

We recognize that a single analysis method is not enough for the diverse range
of shell commands. A simple token-based check is efficient but brittle. A full
semantic model for every command is robust but prohibitively expensive to build
and maintain.

Our hybrid system solves this by dispatching commands to different analysis
engines based on their complexity and risk profile.

## Architecture Overview

The approval process is a pipeline that flows through several distinct stages:

1.  **Dispatch**: The main entry point (`assess_command`) acts as a dispatcher,
    inspecting the command and deciding which analysis engine to use.

2.  **Analysis & Classification**: The command is sent down one of two main paths
    to be classified into a single `CommandCategory`:
    *   **Semantic Model Path (for `git`)**: For complex, high-risk commands
        like `git`, we use a dedicated semantic model. This involves its own
        mini-pipeline:
        1.  **Parse**: The command arguments are parsed into a structured,
            command-specific model (e.g., `GitCommand`).
        2.  **Classify**: Rules are applied to the rich semantic model to determine
            its category (e.g., a `GitCommand` with a `Reset { hard: true }` subcommand
            is classified as `DeletesData`).
    *   **Generic Path (for all other commands)**: For simpler commands or shell
        scripts, we use a more generic approach:
        1.  **Parse**: If the command matches `<shell> -c|-lc "<script>"` and the
            executable name ends with `sh`, we unwrap the script, parse it into an
            AST, and extract a sequence of *word-only* simple commands joined by
            `&&`, `||`, `;`, or `|`. Any script that uses disallowed constructs
            (redirections, subshells, expansions, etc.) is rejected and treated as
            an opaque single command.
        2.  **Normalize & Classify**: Each simple command is normalized (sudo stripping,
            subcommand inference, flag cluster expansion) and classified using a set
            of generic rules.
        3.  **Aggregate**: The categories for all commands in the sequence are
            aggregated, with the most risky category being chosen to represent
            the entire sequence.

3.  **Policy Engine**: The final, single `CommandCategory` from the classification
    stage is fed into a simple policy engine. This engine takes the category,
    the user's configured approval settings (e.g., `AskForApproval::Never`),
    and the sandbox policy, and makes the final `SafetyCheck` decision:
    `AutoApprove`, `AskUser`, or `Reject`.

This layered design provides surgical precision for the commands that need it most,
while remaining efficient and maintainable for the long tail of simpler commands.

## File Structure

-   `mod.rs`: Public API surface, type definitions, and adapters.
-   `parser.rs`: Normalizes argv (sudo stripping, basename tools, subcommand detection),
    unwraps eligible shell invocations, and produces `CommandAst` values.
-   `shell_parser.rs`: Tree-sitter powered shell script parser restricted to word-only
    commands joined by safe operators.
-   `classifier/`: Classifies normalized commands, including the dedicated git
    semantic model (`git_model.rs`, `git_parser.rs`, `git_rules.rs`).
-   `rules/`: Static rule definitions (`command_rules.rs`) plus shared predicates.
-   `rules_index.rs`: Lazily builds fast lookup tables for rules and subcommand-aware
    tools.
-   `policy/`: Aggregates classified commands and maps the highest-risk category through
    sandbox/approval policy into a `CommandDecision`.
-   `patch_*.rs`: Current stubs for a future patch approval engine.
-   `ast.rs` and `ast_matcher.rs`: Shared AST types and helpers used by the parser and
    tests.

## Future Work and Improvements

### Data-Driven Whitelist Expansion

Currently, the `approval` module uses a static, hand-crafted set of rules to define its whitelist of safe commands. While this is a secure approach, it can lead to "prompt fatigue" when users frequently use commands that are not on the whitelist and are therefore prompted for approval.

To address this, we plan to implement a data-driven approach to expanding the whitelist. This will involve:

1.  **Telemetry**: With user consent, collect anonymized data on `Unrecognized` commands, including their frequency, user approval/denial rates, and common usage patterns.
2.  **Weighted Scoring**: Develop a weighted scoring model to identify the best candidates for whitelisting. This model will be designed to be resilient to "prompt fatigue" by giving more weight to signals like explicit denials ("Grumble Factor"), command complexity, and session velocity.
3.  **Review**: Perform a focused security analysis on each top candidate to define a "safe subset" of its functionality.
4.  **Implement**: Add a new rule or semantic model to the `approval` module for the safe subset.
5.  **Iterate**: Continuously repeat the process to improve the approval system over time.

This will create a virtuous cycle where the approval system becomes smarter and less intrusive over time as we gather more data on how the tool is used in the real world.

## Status and Follow-Ups

-   **Patch safety is not yet implemented.** `assess_patch_safety` currently returns a
    placeholder decision and will be fleshed out with dedicated rules.
-   **Sequential execution for joined commands** is a potential optimization: instead of
    rejecting an entire pipeline when one element is unsafe, we may stream execution up
    to the first deniable command to reduce repeated approval prompts while preserving
    safety guarantees.
-   **Data-driven rule expansion** (described above) will refine the whitelist and reduce
    user friction once telemetry, scoring, and review workflows are in place.
'''
