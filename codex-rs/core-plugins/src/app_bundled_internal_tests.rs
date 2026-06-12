use codex_config::HookHandlerConfig;
use codex_plugin::PluginHookSourceKind;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde_json::json;
use std::fs;
use tempfile::TempDir;
use tempfile::tempdir;

use crate::app_bundled_internal::is_app_bundled_internal_candidate;
use crate::app_bundled_internal::test_support::TestAuthenticatedResources;
use crate::app_bundled_internal::test_support::load;

#[test]
fn loads_designated_hook_declaration_and_script_from_authenticated_resources() {
    let fixture = InternalHookFixture::new();
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");
    let plugin_data_root = fixture.absolute("data/computer-use");

    let sources = load(&fixture.resources, &plugin_id, &plugin_data_root).expect("load hooks");

    let [source] = sources.as_slice() else {
        panic!("expected one hook source");
    };
    assert_eq!(source.kind, PluginHookSourceKind::AppBundledInternal);
    assert_eq!(source.plugin_root, fixture.absolute(PLUGIN_RELATIVE_ROOT));
    assert_eq!(
        source.source_path,
        fixture.absolute(&format!("{PLUGIN_RELATIVE_ROOT}/hooks/hooks.json"))
    );
    assert_eq!(source.plugin_data_root, plugin_data_root);
    assert!(
        fixture
            .absolute(&format!("{PLUGIN_RELATIVE_ROOT}/{COMPUTER_USE_EXECUTABLE}"))
            .is_file()
    );
    let HookHandlerConfig::Command {
        command,
        command_windows,
        ..
    } = &source.hooks.stop[0].hooks[0]
    else {
        panic!("expected command hook");
    };
    assert_eq!(
        command,
        &format!("\"${{PLUGIN_ROOT}}/{COMPUTER_USE_EXECUTABLE}\" codex-stop-hook")
    );
    assert_eq!(
        command_windows.as_deref(),
        Some("\"%PLUGIN_ROOT%\\bin\\SkyComputerUseClient.exe\" codex-stop-hook")
    );
    let HookHandlerConfig::Command {
        command: subagent_command,
        command_windows: subagent_command_windows,
        ..
    } = &source.hooks.subagent_stop[0].hooks[0]
    else {
        panic!("expected SubagentStop command hook");
    };
    assert_eq!(subagent_command, command);
    assert_eq!(subagent_command_windows, command_windows);
}

#[test]
fn rejects_incomplete_stop_event_contract() {
    let fixture = InternalHookFixture::new();
    fixture.write_json(
        &format!("{PLUGIN_RELATIVE_ROOT}/hooks/hooks.json"),
        json!({
            "hooks": {
                "Stop": [computer_use_hook_group()]
            }
        }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("missing SubagentStop hook must fail closed");

    assert_eq!(error.stage, "hook contract");
}

#[test]
fn rejects_spoofed_marketplace_identity() {
    let fixture = InternalHookFixture::new();
    fixture.write_json(
        MARKETPLACE_RELATIVE_PATH,
        json!({
            "name": "openai-bundled",
            "plugins": [{
                "name": "computer-use",
                "source": { "source": "local", "path": "./plugins/spoof" }
            }]
        }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("spoofed marketplace must fail");

    assert_eq!(error.stage, "marketplace identity");
}

#[test]
fn rejects_missing_referenced_hook_file() {
    let fixture = InternalHookFixture::new();
    fs::remove_file(
        fixture
            .absolute(&format!("{PLUGIN_RELATIVE_ROOT}/{COMPUTER_USE_EXECUTABLE}"))
            .as_path(),
    )
    .expect("remove script");
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("missing script must fail");

    assert_eq!(error.stage, "hook resource containment");
}

#[test]
fn rejects_hook_command_target_not_listed_in_registry() {
    let fixture = InternalHookFixture::new();
    fixture.write(
        &format!("{PLUGIN_RELATIVE_ROOT}/hooks/other.sh"),
        "#!/bin/sh\n",
    );
    fixture.write_json(
        "plugins/app-bundled-internal-hooks.json",
        json!({
            "schemaVersion": 1,
            "plugins": [{
                "pluginId": "computer-use@openai-bundled",
                "hookDeclarations": ["hooks/hooks.json"],
                "referencedFiles": ["hooks/other.sh"]
            }]
        }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("unlisted hook command target must fail closed");

    assert_eq!(error.stage, "hook command containment");
}

#[test]
fn rejects_shell_payload_after_bundled_executable() {
    let fixture = InternalHookFixture::new();
    fixture.write_json(
        &format!("{PLUGIN_RELATIVE_ROOT}/hooks/hooks.json"),
        json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": format!(
                            "\"${{PLUGIN_ROOT}}/{COMPUTER_USE_EXECUTABLE}\" codex-stop-hook ; payload"
                        ),
                        "commandWindows": "\"%PLUGIN_ROOT%\\bin\\SkyComputerUseClient.exe\" codex-stop-hook ; payload",
                        "timeout": 10
                    }]
                }]
            }
        }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("shell suffix must fail closed");

    assert_eq!(error.stage, "hook command containment");
}

#[test]
fn rejects_empty_internal_hook_opt_in() {
    let fixture = InternalHookFixture::new();
    fixture.write_json(
        "plugins/app-bundled-internal-hooks.json",
        json!({
            "schemaVersion": 1,
            "plugins": [{
                "pluginId": "computer-use@openai-bundled",
                "hookDeclarations": [],
                "referencedFiles": []
            }]
        }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("empty internal hook opt-in must fail closed");

    assert_eq!(error.stage, "registry identity");
}

#[test]
fn accepts_signed_registry_without_an_internal_hook_opt_in() {
    let fixture = InternalHookFixture::new();
    fixture.write_json(
        "plugins/app-bundled-internal-hooks.json",
        json!({ "schemaVersion": 1, "plugins": [] }),
    );
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let sources = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect("signed negative registry result");

    assert!(sources.is_empty());
}

#[test]
fn rejects_distribution_reverification_failure() {
    let mut fixture = InternalHookFixture::new();
    fixture.resources.reverify_succeeds = false;
    let plugin_id = PluginId::parse("computer-use@openai-bundled").expect("plugin id");

    let error = load(
        &fixture.resources,
        &plugin_id,
        &fixture.absolute("data/computer-use"),
    )
    .expect_err("reverification must fail closed");

    assert_eq!(error.stage, "distribution reverification");
}

#[test]
fn only_exact_core_allowlist_identity_is_internal() {
    assert!(is_app_bundled_internal_candidate(
        &PluginId::parse("computer-use@openai-bundled").expect("plugin id")
    ));
    assert!(!is_app_bundled_internal_candidate(
        &PluginId::parse("computer-use@spoofed").expect("plugin id")
    ));
    assert!(!is_app_bundled_internal_candidate(
        &PluginId::parse("spoofed@openai-bundled").expect("plugin id")
    ));
}

const PLUGIN_RELATIVE_ROOT: &str = "plugins/openai-bundled/plugins/computer-use";
const MARKETPLACE_RELATIVE_PATH: &str = "plugins/openai-bundled/.agents/plugins/marketplace.json";
const COMPUTER_USE_EXECUTABLE: &str = "Codex Computer Use.app/Contents/SharedSupport/SkyComputerUseClient.app/Contents/MacOS/SkyComputerUseClient";

struct InternalHookFixture {
    _temp: TempDir,
    root: AbsolutePathBuf,
    resources: TestAuthenticatedResources,
}

impl InternalHookFixture {
    fn new() -> Self {
        let temp = tempdir().expect("tempdir");
        let root =
            AbsolutePathBuf::try_from(fs::canonicalize(temp.path()).expect("canonical tempdir"))
                .expect("absolute tempdir");
        let fixture = Self {
            resources: TestAuthenticatedResources::new(root.clone()),
            _temp: temp,
            root,
        };
        fixture.write_json(
            "plugins/app-bundled-internal-hooks.json",
            json!({
                "schemaVersion": 1,
                "plugins": [{
                    "pluginId": "computer-use@openai-bundled",
                    "hookDeclarations": ["hooks/hooks.json"],
                    "referencedFiles": [
                        COMPUTER_USE_EXECUTABLE,
                        "bin/SkyComputerUseClient.exe"
                    ]
                }]
            }),
        );
        fixture.write_json(
            MARKETPLACE_RELATIVE_PATH,
            json!({
                "name": "openai-bundled",
                "plugins": [{
                    "name": "computer-use",
                    "source": {
                        "source": "local",
                        "path": "./plugins/computer-use"
                    },
                    "policy": { "installation": "AVAILABLE" },
                    "category": "Productivity"
                }],
                "interface": { "displayName": "OpenAI Bundled" }
            }),
        );
        fixture.write_json(
            &format!("{PLUGIN_RELATIVE_ROOT}/.codex-plugin/plugin.json"),
            json!({
                "name": "computer-use",
                "version": "1.0.0",
                "skills": "./skills/",
                "hooks": "./hooks/hooks.json"
            }),
        );
        fixture.write_json(
            &format!("{PLUGIN_RELATIVE_ROOT}/hooks/hooks.json"),
            json!({
                "hooks": {
                    "Stop": [computer_use_hook_group()],
                    "SubagentStop": [computer_use_hook_group()]
                }
            }),
        );
        fixture.write(
            &format!("{PLUGIN_RELATIVE_ROOT}/bin/SkyComputerUseClient.exe"),
            "fake executable\n",
        );
        fixture.write(
            &format!("{PLUGIN_RELATIVE_ROOT}/{COMPUTER_USE_EXECUTABLE}"),
            "fake Mach-O\n",
        );
        fixture.write(
            &format!("{PLUGIN_RELATIVE_ROOT}/skills/computer-use/SKILL.md"),
            "# bundled skill should not be selected\n",
        );
        fixture
    }

    fn absolute(&self, relative_path: &str) -> AbsolutePathBuf {
        self.root.join(relative_path)
    }

    fn write_json(&self, relative_path: &str, value: serde_json::Value) {
        self.write(relative_path, &format!("{value}\n"));
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.root.join(relative_path);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path.as_path(), contents).expect("write fixture");
    }
}

fn computer_use_hook_group() -> serde_json::Value {
    json!({
        "hooks": [{
            "type": "command",
            "command": format!(
                "\"${{PLUGIN_ROOT}}/{COMPUTER_USE_EXECUTABLE}\" codex-stop-hook"
            ),
            "commandWindows": "\"%PLUGIN_ROOT%\\bin\\SkyComputerUseClient.exe\" codex-stop-hook",
            "timeout": 10
        }]
    })
}
