use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PersonalitiesListParams;
use codex_app_server_protocol::PersonalitiesListResponse;
use codex_app_server_protocol::PersonalityScope;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

fn write_personality(
    root: &std::path::Path,
    name: &str,
    description: &str,
    body: &str,
) -> Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
    )?;
    Ok(())
}

async fn request_personalities_list(
    mcp: &mut McpProcess,
    cwd: &std::path::Path,
) -> Result<PersonalitiesListResponse> {
    let request_id = mcp
        .send_personalities_list_request(PersonalitiesListParams {
            cwds: Some(vec![cwd.to_path_buf().try_into()?]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

#[tokio::test]
async fn personalities_list_returns_builtin_and_file_backed_personalities() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo = TempDir::new()?;
    let repo_personalities = repo.path().join(".codex/personalities");
    let user_personalities = codex_home.path().join("personalities");

    write_personality(
        &user_personalities,
        "night-owl",
        "User personality",
        "User instructions",
    )?;
    write_personality(
        &repo_personalities,
        "night-owl",
        "Repo personality",
        "Repo instructions",
    )?;
    write_personality(
        &repo_personalities,
        "ship-it",
        "Repo only",
        "Ship it instructions",
    )?;
    std::fs::write(
        repo_personalities.join("broken.md"),
        "# missing frontmatter\n",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let PersonalitiesListResponse { data } =
        request_personalities_list(&mut mcp, repo.path()).await?;

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].cwd, repo.path().to_path_buf());

    let personalities = &data[0].personalities;
    let names = personalities
        .iter()
        .map(|personality| personality.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["friendly", "none", "pragmatic", "night-owl", "ship-it"]
    );
    assert!(personalities.iter().any(|personality| {
        personality.name == "friendly"
            && personality.is_built_in
            && personality.scope == PersonalityScope::Builtin
    }));
    assert!(personalities.iter().any(|personality| {
        personality.name == "pragmatic"
            && personality.is_built_in
            && personality.scope == PersonalityScope::Builtin
    }));
    assert!(personalities.iter().any(|personality| {
        personality.name == "none"
            && personality.is_built_in
            && personality.scope == PersonalityScope::Builtin
    }));

    let night_owl = personalities
        .iter()
        .find(|personality| personality.name == "night-owl")
        .ok_or_else(|| anyhow!("night-owl personality missing"))?;
    assert_eq!(night_owl.description, "Repo personality");
    assert_eq!(night_owl.scope, PersonalityScope::Repo);
    assert!(!night_owl.is_built_in);

    let ship_it = personalities
        .iter()
        .find(|personality| personality.name == "ship-it")
        .ok_or_else(|| anyhow!("ship-it personality missing"))?;
    assert_eq!(ship_it.description, "Repo only");
    assert_eq!(ship_it.scope, PersonalityScope::Repo);
    assert!(!ship_it.is_built_in);

    assert!(
        personalities
            .iter()
            .all(|personality| personality.name != "broken")
    );
    assert_eq!(
        personalities
            .iter()
            .filter(|personality| personality.name == "night-owl")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn personalities_list_uses_cached_catalog_for_repeated_requests() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo = TempDir::new()?;
    let repo_personalities = repo.path().join(".codex/personalities");

    write_personality(
        &repo_personalities,
        "night-owl",
        "Original personality",
        "Original instructions",
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let first = request_personalities_list(&mut mcp, repo.path()).await?;
    let first_entry = &first.data[0];
    let first_night_owl = first_entry
        .personalities
        .iter()
        .find(|personality| personality.name == "night-owl")
        .ok_or_else(|| anyhow!("night-owl personality missing on first request"))?;
    assert_eq!(first_night_owl.description, "Original personality");

    write_personality(
        &repo_personalities,
        "night-owl",
        "Updated personality",
        "Updated instructions",
    )?;
    write_personality(
        &repo_personalities,
        "fresh-file",
        "New personality",
        "New instructions",
    )?;

    let second = request_personalities_list(&mut mcp, repo.path()).await?;
    let second_entry = &second.data[0];
    let second_night_owl = second_entry
        .personalities
        .iter()
        .find(|personality| personality.name == "night-owl")
        .ok_or_else(|| anyhow!("night-owl personality missing on second request"))?;
    assert_eq!(second_night_owl.description, "Original personality");
    assert!(
        second_entry
            .personalities
            .iter()
            .all(|personality| personality.name != "fresh-file")
    );

    Ok(())
}

#[tokio::test]
async fn personalities_list_evicts_old_catalogs_when_cache_grows() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut repos = Vec::new();

    for index in 0..40 {
        let repo = TempDir::new()?;
        let repo_personalities = repo.path().join(".codex/personalities");
        write_personality(
            &repo_personalities,
            "night-owl",
            &format!("Personality {index}"),
            &format!("Instructions {index}"),
        )?;
        repos.push(repo);
    }

    let first_repo = repos
        .first()
        .ok_or_else(|| anyhow!("expected at least one repo"))?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let first = request_personalities_list(&mut mcp, first_repo.path()).await?;
    let first_entry = &first.data[0];
    let first_night_owl = first_entry
        .personalities
        .iter()
        .find(|personality| personality.name == "night-owl")
        .ok_or_else(|| anyhow!("night-owl personality missing on first request"))?;
    assert_eq!(first_night_owl.description, "Personality 0");

    for repo in repos.iter().skip(1) {
        request_personalities_list(&mut mcp, repo.path()).await?;
    }

    write_personality(
        &first_repo.path().join(".codex/personalities"),
        "night-owl",
        "Updated personality",
        "Updated instructions",
    )?;

    let reloaded = request_personalities_list(&mut mcp, first_repo.path()).await?;
    let reloaded_entry = &reloaded.data[0];
    let reloaded_night_owl = reloaded_entry
        .personalities
        .iter()
        .find(|personality| personality.name == "night-owl")
        .ok_or_else(|| anyhow!("night-owl personality missing after cache churn"))?;
    assert_eq!(reloaded_night_owl.description, "Updated personality");

    Ok(())
}
