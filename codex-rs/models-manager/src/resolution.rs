/// The result of resolving a requested model against a provider's model policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelResolution {
    pub model: String,
    pub fallback: Option<UnsupportedModelFallback>,
}

impl ModelResolution {
    pub(crate) fn exact(model: String) -> Self {
        Self {
            model,
            fallback: None,
        }
    }

    pub(crate) fn fallback(requested_model: String, fallback_model: String) -> Self {
        Self {
            model: fallback_model.clone(),
            fallback: Some(UnsupportedModelFallback {
                requested_model,
                fallback_model,
            }),
        }
    }
}

/// Describes a provider-directed fallback from an unsupported requested model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedModelFallback {
    pub requested_model: String,
    pub fallback_model: String,
}
