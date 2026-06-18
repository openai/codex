use codex_config::ConfigLayerStack;
use codex_core_skills::SkillsLoadInput;
use codex_utils_absolute_path::AbsolutePathBuf;

/// Host inputs needed to discover local skills for one effective config.
#[derive(Clone, Debug)]
pub struct HostSkillsConfig {
    pub(crate) codex_home: AbsolutePathBuf,
    pub(crate) load_input: SkillsLoadInput,
}

impl HostSkillsConfig {
    pub fn new(
        codex_home: AbsolutePathBuf,
        cwd: AbsolutePathBuf,
        config_layer_stack: ConfigLayerStack,
        bundled_skills_enabled: bool,
    ) -> Self {
        Self {
            codex_home,
            load_input: SkillsLoadInput::new(
                cwd,
                Vec::new(),
                config_layer_stack,
                bundled_skills_enabled,
            ),
        }
    }
}

/// Host-supplied configuration used by the skills extension.
#[derive(Clone, Debug)]
pub struct SkillsExtensionConfig {
    /// Whether the available-skills catalog is included in model context.
    pub include_instructions: bool,
    /// Whether bundled skills are eligible for discovery.
    pub bundled_skills_enabled: bool,
    /// Local discovery inputs. `None` disables the host provider.
    pub host: Option<HostSkillsConfig>,
}
