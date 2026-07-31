/// Parameters for listing independently persisted thread sections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListThreadSectionsParams {
    /// Opaque cursor returned by a previous section listing.
    pub cursor: Option<String>,
    /// Maximum number of sections to return.
    pub limit: usize,
}

/// Parameters for creating a thread section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateThreadSectionParams {
    /// User-facing section name.
    pub name: String,
}

/// Parameters for renaming a thread section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenameThreadSectionParams {
    /// Stable identifier of the section to rename.
    pub section_id: String,
    /// Replacement user-facing section name.
    pub name: String,
}

/// Parameters for deleting a thread section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteThreadSectionParams {
    /// Stable identifier of the section to delete.
    pub section_id: String,
}

/// An independently persisted thread section and its user-facing name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredThreadSection {
    /// Stable, server-owned section identifier.
    pub id: String,
    /// User-facing section name.
    pub name: String,
}

/// A cursor-paginated page of independently persisted thread sections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredThreadSectionsPage {
    /// Sections returned for this page.
    pub sections: Vec<StoredThreadSection>,
    /// Opaque cursor to continue listing sections.
    pub next_cursor: Option<String>,
}
