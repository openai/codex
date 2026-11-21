//! Compose managed network policy in source order. Lists replace like-named
//! lists; keyed domain rules update individual selectors under either spelling.

use crate::NetworkDomainPermissionToml::Allow;
use crate::NetworkDomainPermissionToml::Deny;
use crate::NetworkDomainPermissionsToml;
use crate::NetworkRequirementsToml;
use crate::RequirementSource;
use crate::Sourced;
use crate::merge::merge_toml_values;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::net::Ipv6Addr;
use toml::Value;

use super::layer::ComposableRequirementsLayer;
use super::layer::remove_nested_field_and_prune_empty;
use super::layer::remove_top_level_field;
use super::stack::RequirementsCompositionError;

const ALLOWS: &str = "allowed_domains";
const DENIES: &str = "denied_domains";

fn take_domains(table: &mut toml::Table) -> toml::Table {
    [ALLOWS, DENIES, "domains"]
        .into_iter()
        .filter_map(|field| Some((field.to_owned(), table.remove(field)?)))
        .collect()
}

fn parse(
    value: Value,
    source: &RequirementSource,
) -> Result<NetworkRequirementsToml, RequirementsCompositionError> {
    value
        .try_into()
        .map_err(|err: toml::de::Error| RequirementsCompositionError::Parse {
            layer_source: source.clone(),
            message: err.to_string(),
        })
}

fn empty_list(fields: &toml::Table, key: &str) -> bool {
    fields
        .get(key)
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

/// Same-file mirrors already agree as standalone policies. When layered, an
/// omitted list, an empty replacement, and a per-host update are distinct.
fn same_domain_operation(left: &toml::Table, right: &toml::Table) -> bool {
    if left == right || (left.contains_key("domains") && right.contains_key("domains")) {
        return true;
    }
    if left.contains_key("domains") || right.contains_key("domains") {
        return false;
    }
    [(ALLOWS, Allow), (DENIES, Deny)]
        .into_iter()
        .all(|(key, rule)| {
            let identities = |fields: &toml::Table| {
                fields.get(key).and_then(Value::as_array).map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(|pattern| rule.pattern_identity(pattern))
                        .collect::<BTreeSet<_>>()
                })
            };
            identities(left) == identities(right)
        })
}

pub(super) fn compose(
    layers: &mut [ComposableRequirementsLayer],
) -> Result<Option<Box<Sourced<NetworkRequirementsToml>>>, RequirementsCompositionError> {
    let mut merged = Value::Table(toml::Table::new());
    let mut sources = Vec::new();
    let mut inputs = Vec::new();
    let mut nested_inputs = None;
    let mut ambiguous_mirror = None;
    let mut has_empty_map = false;
    let mut has_legacy = false;
    let mut has_nested = false;
    let mut has_socket_paths = false;
    for layer in layers {
        let raw = &mut layer.regular_toml;
        let legacy = remove_top_level_field(raw, "network_proxy")
            .or_else(|| remove_top_level_field(raw, "experimental_network"));
        let nested = remove_nested_field_and_prune_empty(raw, &["permissions", "network"]);
        has_legacy |= legacy.is_some();
        has_nested |= nested.is_some();
        let (mut table, mirror) = match (legacy, nested) {
            (Some(Value::Table(table)), Some(Value::Table(mirror))) => (table, Some(mirror)),
            (Some(Value::Table(table)), None) | (None, Some(Value::Table(table))) => (table, None),
            (None, None) => continue,
            _ => {
                return Err(RequirementsCompositionError::Parse {
                    layer_source: layer.source.clone(),
                    message: "network requirements must be tables".into(),
                });
            }
        };
        has_socket_paths |= std::iter::once(&table)
            .chain(mirror.as_ref())
            .any(|network| {
                network.contains_key("unix_sockets") || network.contains_key("allow_unix_sockets")
            });
        let mut fields = take_domains(&mut table);
        let mut alternative = None;
        if let Some(mut mirror) = mirror {
            let other = take_domains(&mut mirror);
            if fields.contains_key(ALLOWS)
                && other.contains_key(ALLOWS)
                && empty_list(&fields, ALLOWS) != empty_list(&other, ALLOWS)
            {
                return Err(RequirementsCompositionError::Parse {
                    layer_source: layer.source.clone(),
                    message: "legacy and permissions.network requirements specify different allowed_domains replacements".into(),
                });
            }
            if empty_list(&fields, ALLOWS) != empty_list(&other, ALLOWS)
                || empty_list(&fields, DENIES) != empty_list(&other, DENIES)
                || (!fields.is_empty()
                    && !other.is_empty()
                    && !same_domain_operation(&fields, &other))
            {
                ambiguous_mirror = Some(layer.source.clone());
            }
            if fields.is_empty() {
                fields = other;
            } else if !other.is_empty() && fields != other {
                nested_inputs.get_or_insert_with(|| inputs.clone());
                alternative = Some(other);
            }
        }
        if fields
            .get("domains")
            .and_then(Value::as_table)
            .is_some_and(toml::Table::is_empty)
        {
            has_empty_map = true;
        } else if !fields.is_empty() {
            if let Some(nested) = &mut nested_inputs {
                nested.push((
                    layer.source.clone(),
                    alternative.unwrap_or_else(|| fields.clone()),
                ));
            }
            inputs.push((layer.source.clone(), fields));
        }
        sources.push(layer.source.clone());
        merge_toml_values(&mut merged, &Value::Table(table));
    }
    if sources.is_empty() {
        return Ok(None);
    }
    let mixed = sources.len() > 1 && has_legacy && has_nested;
    let source = RequirementSource::composite(sources.into_iter().rev());
    if mixed && has_socket_paths {
        return Err(RequirementsCompositionError::Parse {
            layer_source: source,
            message:
                "Unix socket path rules across sources require a single network table spelling"
                    .into(),
        });
    }
    if inputs.len() > 1
        && let Some(layer_source) = ambiguous_mirror
    {
        return Err(RequirementsCompositionError::Parse {
            layer_source,
            message: "mirrored network domain rules must use the same list replacements or per-host updates when composed with another domain source".into(),
        });
    }
    let mut value = parse(merged, &source)?;
    value.domains = compose_domains(inputs, has_empty_map, &source)?;
    if let Some(nested) = nested_inputs.filter(|inputs| inputs.len() > 1) {
        // Different raw keys in a mirror can change which rules survive replacement.
        let restrictions = |domains: &Option<NetworkDomainPermissionsToml>| {
            let entries = domains.iter().flat_map(|domains| &domains.entries);
            let has_allowlist = entries.clone().any(|(_, &rule)| rule == Allow);
            let denials = entries
                .filter(|(_, rule)| **rule == Deny)
                .map(|(pattern, rule)| rule.pattern_identity(pattern))
                .collect::<BTreeSet<_>>();
            (has_allowlist, denials)
        };
        if restrictions(&compose_domains(nested, has_empty_map, &source)?)
            != restrictions(&value.domains)
        {
            return Err(RequirementsCompositionError::Parse {
                layer_source: source,
                message: "mirrored network domain rules disagree after layer composition".into(),
            });
        }
    }
    if mixed {
        value
            .validate_domain_patterns()
            .map_err(|err| RequirementsCompositionError::Parse {
                layer_source: source.clone(),
                message: err.to_string(),
            })?;
    }
    Ok(Some(Box::new(Sourced::new(value, source))))
}

/// Only the last list of each kind participates. Keyed sources retain their
/// position relative to those two replacements, including a surviving clear.
fn compose_domains(
    inputs: Vec<(RequirementSource, toml::Table)>,
    has_empty_map: bool,
    source: &RequirementSource,
) -> Result<Option<NetworkDomainPermissionsToml>, RequirementsCompositionError> {
    let last_allow = inputs
        .iter()
        .rposition(|(_, fields)| fields.contains_key(ALLOWS));
    let last_deny = inputs
        .iter()
        .rposition(|(_, fields)| fields.contains_key(DENIES));
    let clear = last_allow.filter(|&i| empty_list(&inputs[i].1, ALLOWS));
    let effective = if inputs.len() < 2
        || !inputs
            .iter()
            .any(|(_, fields)| fields.contains_key("domains"))
    {
        // The normal parser handles list replacement and overlapping allow/deny
        // lists, including policies from clients predating keyed domains.
        let mut fields = toml::Table::new();
        for (_, input) in inputs {
            fields.extend(input);
        }
        parse(Value::Table(fields), source)?.domains
    } else {
        let mut domains: BTreeMap<String, BTreeMap<String, _>> = BTreeMap::new();
        let mut list_denials: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut keyed_denials: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut published: BTreeMap<String, BTreeMap<String, _>> = BTreeMap::new();
        for (i, (source, mut fields)) in inputs.into_iter().enumerate() {
            if Some(i) != last_allow {
                fields.remove(ALLOWS);
            }
            if Some(i) != last_deny {
                fields.remove(DENIES);
            }
            let lists = !fields.contains_key("domains");
            let rules = parse(Value::Table(fields), &source)?
                .domains
                .unwrap_or_default();
            let mut updates: BTreeMap<String, BTreeMap<String, _>> = BTreeMap::new();
            for (pattern, rule) in rules.entries {
                let selector = match rule {
                    Allow => codex_network_proxy::canonical_review_pattern(&pattern),
                    Deny => codex_network_proxy::canonical_deny_pattern(&pattern),
                }
                .map_err(|err| RequirementsCompositionError::Parse {
                    layer_source: source.clone(),
                    message: err.to_string(),
                })?;
                if rule == Allow && clear.is_some_and(|clear| i < clear) {
                    continue;
                }
                // A map replaces a denial only by its exact TOML key. A list
                // updating this selector supersedes the previous map entries.
                if lists {
                    keyed_denials.remove(&selector);
                    if rule == Deny {
                        list_denials
                            .entry(selector.clone())
                            .or_default()
                            .push(pattern.clone());
                    }
                } else if rule == Deny {
                    keyed_denials
                        .entry(selector.clone())
                        .or_default()
                        .insert(pattern.clone());
                } else if let Some(denials) = keyed_denials.get_mut(&selector) {
                    denials.remove(&pattern);
                }
                updates.entry(selector).or_default().insert(pattern, rule);
            }
            for (selector, mut entries) in updates {
                // A surviving deny list still wins when a later allow list names
                // the host, even if a keyed grant was published between the lists.
                if lists && let Some(denials) = list_denials.get(&selector) {
                    entries.extend(denials.iter().cloned().map(|pattern| (pattern, Deny)));
                }
                let spellings = published.entry(selector.clone()).or_default();
                spellings.extend(entries.clone());
                if entries.values().any(|&rule| rule == Deny) {
                    // A shadowed grant still pins the managed allowlist for a profile
                    // that forbids user additions. Preserve the raw keys the parser kept.
                    entries.clone_from(spellings);
                } else if let Ok(ip) = selector.parse::<Ipv6Addr>()
                    && let Some(inherited) = domains.remove(&selector)
                    && inherited.values().all(|&rule| rule == Allow)
                {
                    // The proxy matches IPv6 grants as text. Apply a higher wildcard
                    // port to inherited spellings to revoke their local-binding exception.
                    let wildcard_port = codex_network_proxy::is_non_public_ip(ip.into())
                        && entries.keys().any(|pattern| pattern.contains(['*', '?']));
                    for mut pattern in inherited.into_keys() {
                        if wildcard_port {
                            let host = codex_network_proxy::normalize_host(&pattern);
                            pattern = format!("[{host}]:*");
                        }
                        entries.insert(pattern, Allow);
                    }
                }
                domains.insert(selector, entries);
            }
        }
        let mut entries: BTreeMap<_, _> = domains.into_values().flatten().collect();
        for pattern in keyed_denials.into_values().flatten() {
            entries.insert(pattern, Deny);
        }
        Some(NetworkDomainPermissionsToml { entries })
    };
    Ok(effective
        .or_else(|| (clear.is_some() || has_empty_map).then(NetworkDomainPermissionsToml::default)))
}
