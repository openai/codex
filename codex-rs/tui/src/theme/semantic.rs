
/// Semantic color tokens that abstract the actual color values
/// This allows for consistent theming across the entire UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticColor {
    /// Primary color (e.g., main actions, highlights)
    Primary,
    
    /// Secondary color for less prominent elements
    Secondary,
    
    /// Success states and positive feedback
    Success,
    
    /// Error states and negative feedback
    Error,
    
    /// Warning states and caution indicators
    Warning,
    
    /// Informational elements
    Info,
    
    /// Main text color
    Text,
    
    /// Muted/secondary text
    TextMuted,
    
    /// Disabled or very subtle text
    TextDisabled,
    
    /// Main background color
    Background,
    
    /// Surface/card/panel background
    Surface,
    
    /// Elevated surface (e.g., modals, popups)
    SurfaceElevated,
    
    /// Border color for UI elements
    Border,
    
    /// Focused border color
    BorderFocused,
    
    /// Accent color for special highlights
    Accent,
    
    /// Selection/highlight background
    Selection,
    
    /// Code/monospace text color
    CodeText,
    
    /// Code block background
    CodeBackground,
    
    /// Diff addition color
    DiffAdd,
    
    /// Diff deletion color
    DiffRemove,
    
    /// Diff modification color
    DiffModify,
    
    /// Link/URL color
    Link,
    
    /// Shimmer/animation highlight color
    ShimmerHigh,
    
    /// Shimmer/animation mid color
    ShimmerMid,
    
    /// Shimmer/animation low color
    ShimmerLow,
    
    /// Tool/command color
    Tool,
    
    /// Header/title color
    Header,
}

impl SemanticColor {
    /// Returns a human-readable description of the semantic color
    pub fn description(&self) -> &'static str {
        match self {
            Self::Primary => "Primary brand color",
            Self::Secondary => "Secondary color",
            Self::Success => "Success state color",
            Self::Error => "Error state color",
            Self::Warning => "Warning state color",
            Self::Info => "Informational color",
            Self::Text => "Main text color",
            Self::TextMuted => "Muted text color",
            Self::TextDisabled => "Disabled text color",
            Self::Background => "Main background color",
            Self::Surface => "Surface background color",
            Self::SurfaceElevated => "Elevated surface color",
            Self::Border => "Border color",
            Self::BorderFocused => "Focused border color",
            Self::Accent => "Accent highlight color",
            Self::Selection => "Selection background color",
            Self::CodeText => "Code text color",
            Self::CodeBackground => "Code background color",
            Self::DiffAdd => "Diff addition color",
            Self::DiffRemove => "Diff deletion color",
            Self::DiffModify => "Diff modification color",
            Self::Link => "Link color",
            Self::ShimmerHigh => "Shimmer highlight color",
            Self::ShimmerMid => "Shimmer mid-tone color",
            Self::ShimmerLow => "Shimmer low-tone color",
            Self::Tool => "Tool/command color",
            Self::Header => "Header/title color",
        }
    }
    
    /// Returns all semantic color variants for iteration
    pub fn all() -> &'static [SemanticColor] {
        &[
            Self::Primary,
            Self::Secondary,
            Self::Success,
            Self::Error,
            Self::Warning,
            Self::Info,
            Self::Text,
            Self::TextMuted,
            Self::TextDisabled,
            Self::Background,
            Self::Surface,
            Self::SurfaceElevated,
            Self::Border,
            Self::BorderFocused,
            Self::Accent,
            Self::Selection,
            Self::CodeText,
            Self::CodeBackground,
            Self::DiffAdd,
            Self::DiffRemove,
            Self::DiffModify,
            Self::Link,
            Self::ShimmerHigh,
            Self::ShimmerMid,
            Self::ShimmerLow,
            Self::Tool,
            Self::Header,
        ]
    }
}