use codex_app_server_protocol::AppBranding as ApiAppBranding;
use codex_app_server_protocol::AppInfo as ApiAppInfo;
use codex_app_server_protocol::AppMetadata as ApiAppMetadata;
use codex_app_server_protocol::AppReview as ApiAppReview;
use codex_app_server_protocol::AppScreenshot as ApiAppScreenshot;
use codex_connectors::AppBranding;
use codex_connectors::AppInfo;
use codex_connectors::AppMetadata;
use codex_connectors::AppReview;
use codex_connectors::AppScreenshot;

/// Converts the app-server wire type owned by `codex-app-server-protocol` into connector-domain
/// app metadata owned by `codex-connectors`.
///
/// The types stay separate so app-server protocol ownership does not leak into the connector
/// domain crate. Because this crate owns neither type, Rust's orphan rules require an explicit
/// conversion function instead of a `From` implementation.
pub(crate) fn app_info_from_api(app: ApiAppInfo) -> AppInfo {
    let ApiAppInfo {
        id,
        name,
        description,
        logo_url,
        logo_url_dark,
        icon_assets,
        icon_dark_assets,
        distribution_channel,
        branding,
        app_metadata,
        labels,
        install_url,
        is_accessible,
        is_enabled,
        plugin_display_names,
    } = app;
    AppInfo {
        id,
        name,
        description,
        logo_url,
        logo_url_dark,
        icon_assets,
        icon_dark_assets,
        distribution_channel,
        branding: branding.map(app_branding_from_api),
        app_metadata: app_metadata.map(app_metadata_from_api),
        labels,
        install_url,
        is_accessible,
        is_enabled,
        plugin_display_names,
    }
}

fn app_branding_from_api(branding: ApiAppBranding) -> AppBranding {
    let ApiAppBranding {
        category,
        developer,
        website,
        privacy_policy,
        terms_of_service,
        is_discoverable_app,
    } = branding;
    AppBranding {
        category,
        developer,
        website,
        privacy_policy,
        terms_of_service,
        is_discoverable_app,
    }
}

fn app_review_from_api(review: ApiAppReview) -> AppReview {
    let ApiAppReview { status } = review;
    AppReview { status }
}

fn app_screenshot_from_api(screenshot: ApiAppScreenshot) -> AppScreenshot {
    let ApiAppScreenshot {
        url,
        file_id,
        user_prompt,
    } = screenshot;
    AppScreenshot {
        url,
        file_id,
        user_prompt,
    }
}

fn app_metadata_from_api(metadata: ApiAppMetadata) -> AppMetadata {
    let ApiAppMetadata {
        review,
        categories,
        sub_categories,
        seo_description,
        screenshots,
        developer,
        version,
        version_id,
        version_notes,
        first_party_type,
        first_party_requires_install,
        show_in_composer_when_unlinked,
    } = metadata;
    AppMetadata {
        review: review.map(app_review_from_api),
        categories,
        sub_categories,
        seo_description,
        screenshots: screenshots.map(|screenshots| {
            screenshots
                .into_iter()
                .map(app_screenshot_from_api)
                .collect()
        }),
        developer,
        version,
        version_id,
        version_notes,
        first_party_type,
        first_party_requires_install,
        show_in_composer_when_unlinked,
    }
}
