use ts_rs::TS;

#[derive(TS)]
#[ts(export_to = "issue_317/")]
struct VariantId(u32);

#[derive(TS)]
#[ts(export_to = "issue_317/")]
struct VariantOverview {
    id: u32,
    name: String,
}

#[derive(TS)]
#[ts(export, export_to = "issue_317/")]
struct Container {
    variants: Vec<(VariantId, VariantOverview)>,
}
