//! Serialize prepared snapshot values and apply export-name policy without executing shell code.
//! Native declarations stay unchanged unless the credential stage supplies a replacement.

use super::capture::CapturedSnapshot;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::shell_environment::create_env_from_vars;

impl CapturedSnapshot<'_> {
    /// Render native functions, options, and aliases; the executor restores environment separately.
    pub fn render_state(&self) -> String {
        format!("{}{}", self.state, self.aliases)
    }

    /// Persist only exports admitted by the policy. Explicit overrides are supplied
    /// by the command environment; never retain their original captured values.
    pub fn render_script(&self, policy: &ShellEnvironmentPolicy) -> String {
        let allowed = create_env_from_vars(
            self.exports
                .iter()
                .map(|export| (export.key.to_string(), String::new())),
            policy,
            /*thread_id*/ None,
        );
        let mut script = self.render_state();
        script.push_str("# exports (native declarations)\n");
        for export in &self.exports {
            if allowed.contains_key(export.key) && !policy.r#set.contains_key(export.key) {
                script.push_str(&export.source);
            }
        }
        script
    }
}

#[derive(Default)]
pub(super) struct Value {
    pub parts: Vec<ValuePart>,
}

pub(super) enum ValuePart {
    Literal(String),
    Credential { key: String },
}

pub(super) enum Export<'a> {
    Captured(&'a str),
    Assignment {
        declaration: &'a str,
        value: Value,
    },
    Array {
        prefix: &'a str,
        elements: Vec<Value>,
        suffix: &'a str,
    },
    ArrayBinding {
        key: &'a str,
        declaration: &'a str,
        suffix: &'a str,
    },
}

impl Value {
    fn render(&self) -> Option<String> {
        let mut output = String::new();
        for part in &self.parts {
            match part {
                ValuePart::Literal(value) => {
                    output.push_str(shlex::try_quote(value).ok()?.as_ref())
                }
                ValuePart::Credential { key } => {
                    output.push_str(&format!("\"${{{key}-}}\""));
                }
            }
        }
        if output.is_empty() {
            output.push_str("''");
        }
        Some(output)
    }
}

pub(super) fn render(state: &str, aliases: &str, exports: &[Export<'_>]) -> Option<String> {
    let mut output = format!("{state}{aliases}");
    if !exports.is_empty() {
        output.push_str("# exports (native declarations)\n");
    }
    for export in exports {
        match export {
            Export::Captured(source) => output.push_str(source),
            Export::Assignment { declaration, value } => {
                output.push_str(&format!("{declaration}={}\n", value.render()?));
            }
            Export::Array {
                prefix,
                elements,
                suffix,
            } => {
                let elements = elements
                    .iter()
                    .map(Value::render)
                    .collect::<Option<Vec<_>>>()?;
                output.push_str(&format!("{prefix}({}){suffix}", elements.join(" ")));
            }
            Export::ArrayBinding {
                key,
                declaration,
                suffix,
            } => {
                output.push_str(&format!(
                    "if [ \"${{{key}+x}}\" = x ]; then\n{declaration}{}\nfi\n",
                    suffix.trim_end()
                ));
            }
        }
    }
    Some(output)
}
