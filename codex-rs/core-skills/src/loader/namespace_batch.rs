use codex_exec_server::FS_FIND_UP_BATCH_MAX_REQUESTS;
use codex_exec_server::FindUpRequest;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use super::MAX_CONCURRENT_NAMESPACE_LOOKUPS;
use super::NamespaceProbeCache;
use super::ResolvedNamespaceLookup;
use super::namespace_find_up_options;
use super::resolve_namespace_lookup;

struct PendingNamespaceLookup {
    resolution: ResolvedNamespaceLookup,
    next_ancestor: Option<PathUri>,
}

async fn prepare_namespace_lookup(
    cache: &NamespaceProbeCache<'_>,
    root: PathUri,
    probe_root_alone: bool,
) -> PendingNamespaceLookup {
    let mut resolution = ResolvedNamespaceLookup {
        root: root.clone(),
        namespace: None,
    };
    let next_ancestor = if probe_root_alone {
        if let Some(namespace) = cache.resolve_root(&root).await {
            resolution.namespace = Some(namespace);
            None
        } else {
            root.parent()
        }
    } else {
        Some(root)
    };
    PendingNamespaceLookup {
        resolution,
        next_ancestor,
    }
}

async fn try_resolve_namespace_lookups_batched(
    cache: &NamespaceProbeCache<'_>,
    lookup_roots: &[(PathUri, bool)],
) -> Option<Vec<ResolvedNamespaceLookup>> {
    let mut prepared = futures::stream::iter(lookup_roots.iter().cloned().enumerate())
        .map(|(index, (root, probe_root_alone))| async move {
            (
                index,
                prepare_namespace_lookup(cache, root, probe_root_alone).await,
            )
        })
        .buffer_unordered(MAX_CONCURRENT_NAMESPACE_LOOKUPS)
        .collect::<Vec<_>>()
        .await;
    prepared.sort_by_key(|(index, _)| *index);
    let mut lookups = prepared
        .into_iter()
        .map(|(_, lookup)| lookup)
        .collect::<Vec<_>>();
    let options = namespace_find_up_options();

    loop {
        let active = lookups
            .iter_mut()
            .enumerate()
            .filter_map(|(index, lookup)| {
                lookup
                    .next_ancestor
                    .take()
                    .map(|search_start| (index, search_start))
            })
            .collect::<Vec<_>>();
        if active.is_empty() {
            break;
        }

        let mut round_results = Vec::with_capacity(active.len());
        for chunk in active.chunks(FS_FIND_UP_BATCH_MAX_REQUESTS) {
            let requests = chunk
                .iter()
                .map(|(_, search_start)| FindUpRequest {
                    start: search_start.clone(),
                    options: options.clone(),
                })
                .collect::<Vec<_>>();
            let results = cache
                .fs
                .find_up_batch(&requests, /*sandbox*/ None)
                .await
                .ok()?;
            if results.len() != requests.len() {
                return None;
            }
            round_results.extend(results);
        }

        let mut matched_manifests = Vec::new();
        for ((lookup_index, _), result) in active.into_iter().zip(round_results) {
            let Ok(outcome) = result else {
                continue;
            };
            if let Some(matched) = outcome.matched {
                matched_manifests.push((lookup_index, matched.ancestor, matched.path));
            }
        }
        let resolved_manifests = futures::stream::iter(matched_manifests)
            .map(|(lookup_index, ancestor, manifest)| async move {
                let namespace = cache.resolve_manifest(&ancestor, &manifest).await;
                (lookup_index, ancestor, namespace)
            })
            .buffered(MAX_CONCURRENT_NAMESPACE_LOOKUPS)
            .collect::<Vec<_>>()
            .await;
        for (lookup_index, ancestor, namespace) in resolved_manifests {
            let lookup = &mut lookups[lookup_index];
            if let Some(namespace) = namespace {
                lookup.resolution.namespace = Some(namespace);
            } else {
                lookup.next_ancestor = ancestor.parent();
            }
        }
    }

    Some(
        lookups
            .into_iter()
            .map(|lookup| lookup.resolution)
            .collect(),
    )
}

pub(super) async fn resolve_namespace_lookups(
    cache: &NamespaceProbeCache<'_>,
    lookup_roots: &[(PathUri, bool)],
) -> Vec<ResolvedNamespaceLookup> {
    if let Some(resolutions) = try_resolve_namespace_lookups_batched(cache, lookup_roots).await {
        return resolutions;
    }
    futures::stream::iter(lookup_roots.iter().cloned())
        .map(|(root, probe_root_alone)| async move {
            resolve_namespace_lookup(cache, root, probe_root_alone).await
        })
        .buffered(MAX_CONCURRENT_NAMESPACE_LOOKUPS)
        .collect()
        .await
}
