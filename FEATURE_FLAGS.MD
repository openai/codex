# Feature Flag Implementation Approaches for Codex

This document discusses approaches for implementing feature flags within the Codex system, covering both internal development flags and user-configurable features.

## 1. Clarification of Scope

It's crucial to distinguish between two types of "feature flags" or controls:

*   **User-Configurable Feature Controls:** These are settings exposed to users to modify the behavior of the system according to their preferences or security requirements. Examples in Codex include:
    *   `approvalMode`: Determines whether the agent requires explicit user approval for actions like applying patches or running commands.
    *   `sandbox_policy`: (Hypothetical, but aligned with `exec` and `safety` crates) Could define the strictness of the execution sandbox.
    These are part of the application's intended functionality and provide users with control over its operation.

*   **Internal Development Feature Flags:** These are temporary switches used by the development team to enable or disable incomplete or experimental features/components *internally*. The primary purposes are:
    *   **Trunk-Based Development:** Allow merging unfinished features into the main codebase without exposing them to users or disrupting stable functionality.
    *   **Canary Releases/Testing:** Enable a new feature for a small subset of internal testers or a specific environment.
    *   **Risk Mitigation:** Quickly disable a problematic feature in production without a full rollback.
    These flags are typically not meant for end-user interaction and are removed once the feature is stable or abandoned.

This document will address mechanisms for both, but with a clear distinction in their implementation and purpose.

## 2. Internal Development Feature Flags

For internal development, a simple configuration-based approach is often sufficient and balances ease of implementation with effectiveness.

### `codex-cli` (TypeScript)

A straightforward method is to use a configuration file (e.g., `config.json` or a dedicated `cli-config.json`) read at startup.

*   **Mechanism:**
    1.  Define a `featureFlags` section within the JSON configuration file.
    2.  The application reads this section at startup and stores the flag states globally or makes them available through a configuration service.

*   **Example `config.json`:**
    ```json
    {
      "someUserSetting": "value",
      "approvalMode": "auto",
      "featureFlags": {
        "experimentalNewParser": true,
        "superCoolUnfinishedUI": false
      }
    }
    ```

*   **Conceptual Code Example (TypeScript):**

    ```typescript
    // config.ts
    import * as fs from 'fs';
    import * as path from 'path';

    interface AppConfig {
      someUserSetting: string;
      approvalMode: string;
      featureFlags: {
        [key: string]: boolean;
      };
    }

    let config: AppConfig;

    export function loadConfig(configPath: string = 'config.json'): void {
      try {
        const rawData = fs.readFileSync(path.resolve(__dirname, configPath), 'utf-8');
        config = JSON.parse(rawData) as AppConfig;
      } catch (error) {
        console.error("Error loading config, using defaults:", error);
        // Define default flags if config is missing or invalid
        config = {
          someUserSetting: 'default',
          approvalMode: 'manual',
          featureFlags: {
            experimentalNewParser: false,
            superCoolUnfinishedUI: false,
          }
        };
      }
    }

    export function isFeatureEnabled(flagName: string): boolean {
      return config?.featureFlags?.[flagName] ?? false;
    }

    // Initialize config on load
    loadConfig();

    // Usage in another file
    // import { isFeatureEnabled } from './config';
    //
    // if (isFeatureEnabled('experimentalNewParser')) {
    //   console.log("Experimental parser is enabled!");
    //   // Use new parser logic
    // } else {
    //   // Use old parser logic
    // }
    ```

### `codex-rs` (Rust)

Flags for `codex-rs` can be managed either through configuration passed from the CLI or via environment variables.

*   **Mechanism 1: Configuration via `ConfigureSession`**
    1.  The `codex-cli` reads its `config.json` (which includes a section for backend feature flags or specific settings that imply features).
    2.  During the `ConfigureSession` operation (or a similar initialization step), the CLI transmits these flag states to `codex-rs`.
    3.  `codex-rs` stores these flags in its `Session` struct or a global configuration struct.

*   **Mechanism 2: Environment Variables**
    1.  Feature flags are set as environment variables when launching the `codex-rs` process (e.g., `CODEX_FEATURE_NEW_ALGORITHM=true`).
    2.  Rust code reads these environment variables at startup using `std::env::var`.

*   **Mechanism 3: Compile-time Features (for larger, more distinct features)**
    1. Rust's Cargo features can be used for conditional compilation. This is suitable for enabling/disabling larger chunks of code, often entire modules.
    2. These are typically enabled/disabled at build time (e.g. `cargo build --features experimental-feature`).

*   **Conceptual Code Example (Rust - Runtime Check, Flags from Config/Session):**

    Assume flags are populated into a `SessionConfig` struct.

    ```rust
    // Part of session.rs or config.rs

    #[derive(Clone, Debug)]
    pub struct FeatureFlags {
        pub enable_experimental_analysis: bool,
        pub use_new_error_reporting: bool,
    }

    impl Default for FeatureFlags {
        fn default() -> Self {
            FeatureFlags {
                enable_experimental_analysis: false, // Default to off
                use_new_error_reporting: false,
            }
        }
    }

    // Example of how it might be stored in Session
    // pub struct Session {
    //     pub feature_flags: FeatureFlags,
    //     // ... other session data
    // }

    // Usage in some_module.rs
    // Assuming 'session' is an instance of the Session struct available in the context.
    //
    // fn process_data(session: &Session, data: &MyData) {
    //     if session.feature_flags.enable_experimental_analysis {
    //         // Use experimental analysis logic
    //         println!("Using experimental analysis.");
    //     } else {
    //         // Use standard analysis logic
    //         println!("Using standard analysis.");
    //     }
    //
    //     if session.feature_flags.use_new_error_reporting {
    //         // report_error_new_way();
    //     } else {
    //         // report_error_old_way();
    //     }
    // }
    ```

*   **Conceptual Code Example (Rust - Environment Variable Check):**

    ```rust
    // config.rs
    use std::env;

    #[derive(Clone, Debug)]
    pub struct RuntimeFeatureFlags {
        pub enable_advanced_logging: bool,
    }

    impl RuntimeFeatureFlags {
        pub fn load() -> Self {
            RuntimeFeatureFlags {
                enable_advanced_logging: env::var("CODEX_FEATURE_ADVANCED_LOGGING")
                    .map_or(false, |val| val.eq_ignore_ascii_case("true")),
            }
        }
    }

    // Usage
    // let features = RuntimeFeatureFlags::load();
    // if features.enable_advanced_logging {
    //     // Initialize advanced logging
    // }
    ```

## 3. User-Configurable Features (as Flags)

These are distinct from internal development flags as they are intended for user control.

### Existing Controls as Flags

*   **`approvalMode`:** As described in `ARCHITECTURE.MD`, this setting directly acts as a feature flag for agent autonomy. Users can toggle between modes (e.g., 'auto', 'manual') to enable or disable the agent's ability to proceed without explicit confirmation.
*   **`sandbox_policy`:** (If implemented) This would allow users to select different levels of sandboxing for executed commands, effectively enabling/disabling certain risky operations based on the chosen policy.

### Leveraging the Multi-Capability Protocol (MCP)

The MCP, designed for extending Codex with new tools and agents, naturally supports a form of feature flagging for these capabilities:

*   **MCP Tools as Features:** Each new tool or agent capability integrated via MCP can be considered an optional "feature."
*   **Configuration-Based Enablement:** A section in `config.json` (read by `codex-cli` and potentially communicated to `codex-rs`) could list MCP tools and allow users to enable or disable them.
    *   This allows users to opt-in to experimental or specialized tools.
    *   It helps manage the complexity of the user interface by not overwhelming users with tools they don't need.
    *   It can also be a way for the backend to signal available tools to the CLI.

*   **Example `config.json` for MCP Tool Flagging:**
    ```json
    {
      "approvalMode": "manual",
      "enabledMcpTools": [
        "codeRefactorTool",
        "documentationGenerator"
        // "experimentalCodeQualityChecker" // User has this commented out, disabling it
      ],
      "featureFlags": { // Internal dev flags
        "useNewCliLayout": false
      }
    }
    ```
    The `codex-rs` backend, when processing tasks, would only offer or utilize MCP tools that are enabled in the user's configuration. The `codex-cli` could also use this list to customize the UI, for example, by only showing commands or options related to enabled tools.

## 4. Trade-offs: Config-Based vs. Dynamic Feature Flag Services

The proposed simple config-based flags are suitable for many scenarios, especially for internal development flags and basic user-configurable features. However, more complex needs might warrant dynamic feature flag services (e.g., LaunchDarkly, Flagsmith).

| Aspect                | Simple Config-Based Flags (Proposed)                                  | Dynamic Feature Flag Services (e.g., LaunchDarkly)                               |
| :-------------------- | :-------------------------------------------------------------------- | :------------------------------------------------------------------------------- |
| **Complexity**        | Low. Easy to implement and manage for smaller teams/projects.         | Higher. Requires integrating an external service, SDKs, and management dashboard. |
| **Dynamic Updates**   | Requires application restart or config reload to change flag states.  | Can update flag states dynamically at runtime without redeploying the application. |
| **Targeting**         | Basic (all users or none, based on the loaded config).                | Advanced. Can target specific user segments, percentages, attributes, etc.       |
| **A/B Testing**       | Not directly supported.                                               | Built-in support for A/B testing and experimentation.                           |
| **Management Overhead** | Low. Managed via config files in the codebase.                      | Higher. Requires managing flags and targeting rules on an external platform.    |
| **Cost**              | None (part of the application).                                       | Typically involves subscription costs for the service.                             |
| **Use Cases**         | Internal development flags, simple user opt-ins, resource-constrained environments. | Complex product rollouts, A/B testing, granular user targeting, real-time control. |

**Recommendation for Codex:**

*   For **internal development flags**, the proposed simple config-based or environment variable approach is likely sufficient and appropriate for `codex-cli` and `codex-rs`.
*   For **user-configurable features**, existing mechanisms like `approvalMode` and the proposed MCP-based enablement via `config.json` offer good control.
*   Dynamic feature flag services might be overkill for the current stage but could be considered in the future if highly dynamic rollouts or sophisticated A/B testing of agent capabilities become a requirement.

By clearly distinguishing the types of flags and their purposes, Codex can implement a flexible and manageable system for both development agility and user customization.
File 'FEATURE_FLAGS.MD' created successfully.
