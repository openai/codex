use crate::common::Reasoning;
use codex_protocol::models::ResponseItem;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchRequest {
    pub id: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<SearchInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<SearchCommands>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<SearchSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SearchInput {
    Text(String),
    Items(Vec<ResponseItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, JsonSchema)]
pub struct SearchCommands {
    /// Query the internet search engine for a given list of queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_query: Option<Vec<SearchQuery>>,
    /// Query the image search engine for a given list of queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_query: Option<Vec<SearchQuery>>,
    /// Open pages by reference id or URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<Vec<OpenOperation>>,
    /// Open links from previously opened pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click: Option<Vec<ClickOperation>>,
    /// Find text patterns in pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find: Option<Vec<FindOperation>>,
    /// Take screenshots of PDF pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<Vec<ScreenshotOperation>>,
    /// Look up prices for the given stock symbols.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finance: Option<Vec<FinanceOperation>>,
    /// Look up weather forecasts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weather: Option<Vec<WeatherOperation>>,
    /// Look up sports schedules and standings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sports: Option<Vec<SportsOperation>>,
    /// Get time for the given UTC offsets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Vec<TimeOperation>>,
    /// Set the length of the response to be returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_length: Option<SearchResponseLength>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SearchQuery {
    /// Search query.
    pub q: String,
    /// Whether to filter by recency, as a number of recent days.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recency: Option<u64>,
    /// Whether to filter by a specific list of domains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct OpenOperation {
    /// Reference id or URL to open.
    pub ref_id: String,
    /// Line number to position the page at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineno: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct ClickOperation {
    /// Reference id containing the numbered link.
    pub ref_id: String,
    /// Numbered link id to open.
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct FindOperation {
    /// Reference id or URL to search within.
    pub ref_id: String,
    /// Text pattern to find.
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct ScreenshotOperation {
    /// Reference id or URL to screenshot.
    pub ref_id: String,
    /// Zero-indexed PDF page number.
    pub pageno: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct FinanceOperation {
    /// Ticker symbol to look up.
    pub ticker: String,
    /// Asset type to look up.
    pub r#type: FinanceAssetType,
    /// ISO 3166-1 alpha-3 country code, "OTC", or "" for cryptocurrency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FinanceAssetType {
    Equity,
    Fund,
    Crypto,
    Index,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct WeatherOperation {
    /// Location in "Country, Area, City" format.
    pub location: String,
    /// Start date in YYYY-MM-DD format. Defaults to today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// Number of days to return. Defaults to 7.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SportsOperation {
    /// Tool name for sports requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<SportsToolName>,
    /// Sports function to call.
    pub r#fn: SportsFunction,
    /// League to look up.
    pub league: SportsLeague,
    /// Team to look up, using the common 3 or 4 letter alias used in broadcasts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    /// Opponent to use with `team` when narrowing the lookup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opponent: Option<String>,
    /// Start date in YYYY-MM-DD format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_from: Option<String>,
    /// End date in YYYY-MM-DD format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_to: Option<String>,
    /// Number of games to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_games: Option<u64>,
    /// Locale for the lookup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SportsToolName {
    Sports,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SportsFunction {
    Schedule,
    Standings,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SportsLeague {
    Nba,
    Wnba,
    Nfl,
    Nhl,
    Mlb,
    Epl,
    Ncaamb,
    Ncaawb,
    Ipl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct TimeOperation {
    /// UTC offset formatted like "+03:00".
    pub utc_offset: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchResponseLength {
    Short,
    Medium,
    Long,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExternalWebAccessMode {
    Cached,
    Indexed,
    Live,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(untagged)]
pub enum ExternalWebAccess {
    Boolean(bool),
    Mode(ExternalWebAccessMode),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SearchSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<ApproximateLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_context_size: Option<SearchContextSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<SearchFilters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_settings: Option<SearchImageSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_callers: Option<Vec<AllowedCaller>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_web_access: Option<ExternalWebAccess>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApproximateLocation {
    pub r#type: LocationType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LocationType {
    Approximate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchContextSize {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SearchFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SearchImageSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_results: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AllowedCaller {
    Direct,
    Shell,
    CodeInterpreter,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SearchResponse {
    pub encrypted_output: Option<String>,
    pub output: String,
    /// Structured result DTOs are passed to clients out-of-band from `output`.
    /// Keep them opaque here so newer result variants remain forward-compatible.
    #[serde(default)]
    pub results: Option<Vec<JsonValue>>,
}
