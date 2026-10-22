//! Bounded catalog partitions for callers migrated to CCA. Other callers keep the default catalog.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModelCatalogOriginator {
    Work,
    WorkWeb,
    WorkDesktop,
    WorkMobile,
    Chat,
}

impl ModelCatalogOriginator {
    pub fn from_originator(originator: &str) -> Option<Self> {
        match originator {
            "codex_work_cca" => Some(Self::Work),
            "codex_work_web" => Some(Self::WorkWeb),
            "codex_work_desktop" => Some(Self::WorkDesktop),
            "codex_work_mobile" => Some(Self::WorkMobile),
            "chatgpt_cca" => Some(Self::Chat),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "codex_work_cca",
            Self::WorkWeb => "codex_work_web",
            Self::WorkDesktop => "codex_work_desktop",
            Self::WorkMobile => "codex_work_mobile",
            Self::Chat => "chatgpt_cca",
        }
    }
}
