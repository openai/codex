//! Attaches host observations to request metadata without changing tool execution.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Sampling,
    Compaction,
}

impl ExecutedToolCalls {
    pub(crate) fn attach_to_compaction_prompt(&self, items: &mut [ResponseItem]) {
        self.attach_to_request(items, &mut HashMap::new(), RequestKind::Compaction);
    }

    pub(crate) fn attach_to_prompt(
        &self,
        items: &mut [ResponseItem],
        retry_cache: &mut ExecutedToolCallCache,
    ) {
        self.attach_to_request(items, retry_cache, RequestKind::Sampling);
    }

    fn attach_to_request(
        &self,
        items: &mut [ResponseItem],
        retry_cache: &mut ExecutedToolCallCache,
        request_kind: RequestKind,
    ) {
        {
            let mut state = self.lock_state();
            let Some(state) = state.as_mut() else {
                // Disabling capture also stops replaying Direct records from history.
                clear_direct_call_metadata(items);
                return;
            };
            for item in items.iter_mut() {
                if let Some(call) = item.id().and_then(|id| state.direct_calls.get(id)) {
                    item.clear_executed_tool_calls();
                    item.append_executed_tool_calls(vec![call.clone()]);
                    if call.has_complete_inventory() {
                        item.mark_tool_calls_complete();
                    }
                }
            }
            // A shortened compaction attempt is not an installed history window.
            // Only ordinary sampling can discard observations absent from the window.
            if request_kind == RequestKind::Sampling && !state.direct_calls.is_empty() {
                let output_ids = items
                    .iter()
                    .filter_map(ResponseItem::id)
                    .collect::<HashSet<_>>();
                state.direct_calls.retain(|id, _| output_ids.contains(id));
            }
            Self::attach_pending_to_prompt_with_state(state, items, retry_cache, request_kind);
        }

        // Argument truncation preserves inventory completeness; omitted calls or
        // names still invalidate completion across the cell's outputs.
        normalize_executed_tool_call_arguments(items);
    }

    fn attach_pending_to_prompt_with_state(
        state: &mut ExecutedToolCallRecorderState,
        items: &mut [ResponseItem],
        retry_cache: &mut ExecutedToolCallCache,
        request_kind: RequestKind,
    ) -> bool {
        // Failed wrappers have no callback; only their bounded bitmap observation survives.
        state.pending_wrapper_origins.clear();
        if state.output_cells.is_empty()
            && state.retained_calls.is_empty()
            && retry_cache.is_empty()
        {
            return false;
        }

        let mut output_counts = HashMap::new();
        let mut input_indices = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            if let Some(call_id) = output_call_id(item) {
                *output_counts.entry(call_id.to_string()).or_insert(0_usize) += 1;
            }
            if let Some(call_id) = input_call_id(item) {
                input_indices
                    .entry(call_id.to_string())
                    .and_modify(|entry: &mut Option<usize>| *entry = None)
                    .or_insert(Some(index));
            }
        }

        let has_truncated_bindings = state
            .retained_calls
            .values()
            .any(|retained| !retained.truncated_call_index_by_id.is_empty())
            || state.cells.values().any(|cell| {
                cell.truncated_metadata_binding_valid
                    && cell.pending_calls.values().any(|call| {
                        matches!(
                            call.arguments(),
                            ExecutedToolCallArguments::Truncated { .. }
                        ) && !call.has_tool_result_metadata()
                    })
            });
        if has_truncated_bindings {
            // A conflicting sibling output invalidates the new late backfill too.
            // Check every known output before cloning any retained calls into this request.
            let mut untrusted_cells = HashSet::new();
            for (index, item) in items.iter().enumerate() {
                let Some(call_id) = output_call_id(item) else {
                    continue;
                };
                let key = (std::mem::discriminant(item), call_id.to_string());
                let retained = state.retained_calls.get(&key);
                let binding = retained
                    .and_then(|retained| {
                        Some((
                            retained.runtime_cell_id.as_ref()?,
                            retained.cell_id.as_deref()?,
                            true,
                        ))
                    })
                    .or_else(|| {
                        let cell_id = state.output_cells.get(call_id)?;
                        let origin = state.cells.get(cell_id)?.originating_call_id.as_deref()?;
                        Some((cell_id, origin, false))
                    });
                let Some((cell_id, origin, previously_retained)) = binding else {
                    continue;
                };
                let empty_inventory = item.executed_tool_call_metadata().is_none_or(|metadata| {
                    metadata.executed_tool_calls.is_none() && metadata.cell_id.is_none()
                });
                let matches_input = match input_indices.get(call_id) {
                    None => previously_retained,
                    Some(input_index) => input_index.is_some_and(|input_index| {
                        input_index < index
                            && code_mode_input_matches_output(
                                &items[input_index],
                                item,
                                origin,
                                cell_id,
                            )
                    }),
                };
                let matches_retry = retained.is_none_or(|retained| {
                    retry_cache
                        .get(&key)
                        .filter(|_| !retained.result_metadata_updated)
                        .is_none_or(|cached| {
                            cached.len() == retained.calls.len()
                                && cached.iter().zip(&retained.calls).all(|(cached, current)| {
                                    cached.name == current.name
                                        && cached.arguments() == current.arguments()
                                })
                        })
                });
                if output_counts.get(call_id) != Some(&1)
                    || !matches_input
                    || !empty_inventory
                    || !matches_retry
                {
                    untrusted_cells.insert(cell_id.clone());
                }
            }
            for cell_id in &untrusted_cells {
                if let Some(cell) = state.cells.get_mut(cell_id) {
                    cell.truncated_metadata_binding_valid = false;
                }
            }
            for retained in state.retained_calls.values_mut() {
                if retained
                    .runtime_cell_id
                    .as_ref()
                    .is_some_and(|id| untrusted_cells.contains(id))
                {
                    retained.clear_late_truncated_metadata();
                }
            }
        }

        // Updated records supersede older retry snapshots.
        retry_cache.retain(|key, _| {
            !state
                .retained_calls
                .get(key)
                .is_some_and(|retained| retained.result_metadata_updated)
        });
        let mut pending_outputs = retry_cache
            .keys()
            .chain(state.retained_calls.keys())
            .cloned()
            .collect::<HashSet<_>>();
        let mut attached = false;
        for index in (0..items.len()).rev() {
            if state.output_cells.is_empty() && pending_outputs.is_empty() {
                break;
            }
            let item = &items[index];
            if has_direct_call_metadata(item) {
                continue;
            }
            let Some(call_id) = output_call_id(item) else {
                continue;
            };
            let key = (std::mem::discriminant(item), call_id.to_string());
            let retained = state.retained_calls.get(&key);
            let mut complete = retained.is_some_and(|retained| retained.complete);
            let mut cell_id = retained.and_then(|retained| retained.cell_id.clone());
            let calls = if let Some(cached) =
                retry_cache.get(&key).or_else(|| retained.map(|r| &r.calls))
            {
                if !pending_outputs.remove(&key) {
                    continue;
                }
                cached.clone()
            } else {
                let mut runtime_cell_id = None;
                let mut calls = Vec::new();
                let mut call_index_by_id = HashMap::new();
                let mut truncated_call_index_by_id = HashMap::new();
                if let Some(output_cell_id) = state.output_cells.remove(call_id)
                    && let Some(cell) = state.cells.get_mut(&output_cell_id)
                {
                    cell_id = cell.originating_call_id.clone();
                    runtime_cell_id = Some(output_cell_id.clone());
                    // Validate the first delta too: a later wait cannot repair a
                    // missing or ambiguous exec/output association.
                    let matches_input =
                        input_indices
                            .get(call_id)
                            .copied()
                            .flatten()
                            .is_some_and(|input_index| {
                                input_index < index
                                    && cell_id.as_deref().is_some_and(|origin| {
                                        code_mode_input_matches_output(
                                            &items[input_index],
                                            item,
                                            origin,
                                            &output_cell_id,
                                        )
                                    })
                            });
                    let valid_output = output_counts.get(call_id) == Some(&1) && matches_input;
                    if !valid_output {
                        cell.completion = CellCompletion::Incomplete;
                        cell.truncated_metadata_binding_valid = false;
                    }
                    let pending_calls = cell.pending_calls.len();
                    for (call_id, call) in cell.pending_calls.drain(..) {
                        if matches!(call.arguments(), ExecutedToolCallArguments::Raw(_)) {
                            call_index_by_id.insert(call_id, calls.len());
                        } else if cell.truncated_metadata_binding_valid
                            && !call.has_tool_result_metadata()
                        {
                            truncated_call_index_by_id.insert(call_id, calls.len());
                        }
                        calls.push(call);
                    }
                    cell.pending_full_argument_bytes = 0;
                    complete = cell.completion == CellCompletion::Complete
                        && (state.can_prove_wait_completion
                            || matches!(item, ResponseItem::CustomToolCallOutput { .. }));
                    let has_retained_truncated_calls =
                        state.retained_calls.values().any(|retained| {
                            retained.runtime_cell_id.as_ref() == Some(&output_cell_id)
                                && !retained.truncated_call_index_by_id.is_empty()
                        });
                    let keep_truncated_binding = cell.truncated_metadata_binding_valid
                        && cell.observed_truncated_call
                        && (!cell.dispatch_closed
                            || !truncated_call_index_by_id.is_empty()
                            || has_retained_truncated_calls);
                    if matches!(
                        cell.completion,
                        CellCompletion::Complete | CellCompletion::Incomplete
                    ) && !keep_truncated_binding
                    {
                        // Retain the original binding while a live cell can dispatch or
                        // a closed cell still has results eligible for late backfill.
                        state.cells.remove(&output_cell_id);
                    }
                    state.pending_nested_calls =
                        state.pending_nested_calls.saturating_sub(pending_calls);
                    state
                        .output_cells
                        .retain(|_, registered_cell_id| registered_cell_id != &output_cell_id);
                }
                if calls.is_empty() && !complete {
                    continue;
                }
                retry_cache.insert(key.clone(), calls.clone());
                state.retained_calls.insert(
                    key,
                    RetainedToolCalls {
                        calls: calls.clone(),
                        complete,
                        cell_id: cell_id.clone(),
                        runtime_cell_id,
                        call_index_by_id,
                        truncated_call_index_by_id,
                        late_truncated_indices: HashSet::new(),
                        result_metadata_updated: false,
                    },
                );
                calls
            };
            let item = &mut items[index];
            item.append_executed_tool_calls(calls);
            if let Some(cell_id) = cell_id {
                item.set_tool_call_cell_id(&cell_id);
            }
            if complete {
                item.mark_tool_calls_complete();
            } else {
                item.clear_tool_calls_complete();
            }
            attached = true;
        }
        // Compaction may retry with a shortened history without installing that history.
        // Keep absent observations until a normal sampling request confirms the live window.
        if request_kind == RequestKind::Sampling && !pending_outputs.is_empty() {
            state
                .retained_calls
                .retain(|key, _| !pending_outputs.contains(key));
        }
        let mut invalid_cells = HashSet::new();
        for index in 0..items.len() {
            let item = &items[index];
            if has_direct_call_metadata(item) {
                continue;
            }
            let Some(call_id) = output_call_id(item) else {
                continue;
            };
            let key = (std::mem::discriminant(item), call_id.to_string());
            let Some(retained) = state.retained_calls.get_mut(&key) else {
                continue;
            };
            let matches_inventory = item.executed_tool_call_metadata().is_some_and(|metadata| {
                metadata.cell_id == retained.cell_id
                    && (metadata.has_same_tool_calls(&retained.calls)
                        || (retained.calls.is_empty() && metadata.executed_tool_calls.is_none()))
            });
            // A verified output may outlive its input after compaction. Any input
            // still present must continue to identify the same exec or wait.
            let matches_input = input_indices.get(call_id).is_none_or(|input_index| {
                input_index.is_some_and(|input_index| {
                    input_index < index
                        && match (&retained.cell_id, &retained.runtime_cell_id) {
                            (Some(origin), Some(runtime_cell)) => code_mode_input_matches_output(
                                &items[input_index],
                                item,
                                origin,
                                runtime_cell,
                            ),
                            _ => true,
                        }
                })
            });
            // Lost or ambiguous evidence cannot become complete again on a retry.
            let intact =
                output_counts.get(call_id) == Some(&1) && matches_inventory && matches_input;
            retained.complete &= intact;
            if !intact && let Some(cell) = retained.runtime_cell_id.clone() {
                invalid_cells.insert(cell);
            }
        }
        for cell_id in &invalid_cells {
            if let Some(cell) = state.cells.get_mut(cell_id) {
                cell.completion = CellCompletion::Incomplete;
                cell.truncated_metadata_binding_valid = false;
            }
        }
        if !invalid_cells.is_empty() {
            for retained in state.retained_calls.values_mut() {
                if retained
                    .runtime_cell_id
                    .as_ref()
                    .is_some_and(|cell_id| invalid_cells.contains(cell_id))
                {
                    retained.complete = false;
                    retained.call_index_by_id.clear();
                    retained.clear_late_truncated_metadata();
                }
            }
        }
        for item in items.iter_mut() {
            if has_direct_call_metadata(item) {
                continue;
            }
            let Some(call_id) = output_call_id(item) else {
                continue;
            };
            let key = (std::mem::discriminant(&*item), call_id.to_string());
            let Some(retained) = state.retained_calls.get(&key) else {
                continue;
            };
            if retained.complete {
                item.mark_tool_calls_complete();
            } else {
                item.clear_tool_calls_complete();
            }
        }

        attached
    }
}

// Direct records already belong to their output; never rebuild them from the Code Mode cache.
fn clear_direct_call_metadata(items: &mut [ResponseItem]) {
    for item in items
        .iter_mut()
        .filter(|item| has_direct_call_metadata(item))
    {
        item.clear_executed_tool_calls();
    }
}

fn has_direct_call_metadata(item: &ResponseItem) -> bool {
    item.executed_tool_call_metadata().is_some_and(|metadata| {
        metadata.cell_id.is_none() && metadata.executed_tool_calls.is_some()
    })
}

fn code_mode_input_matches_output(
    input: &ResponseItem,
    output: &ResponseItem,
    origin: &str,
    runtime_cell: &CellId,
) -> bool {
    match (input, output) {
        (
            ResponseItem::CustomToolCall {
                name,
                namespace,
                call_id,
                ..
            },
            ResponseItem::CustomToolCallOutput { .. },
        ) => {
            call_id == origin
                && crate::tools::code_mode::is_exec_tool_name(&codex_tools::ToolName::new(
                    namespace.clone(),
                    name,
                ))
        }
        (
            ResponseItem::FunctionCall {
                name,
                namespace,
                call_id,
                arguments,
                ..
            },
            ResponseItem::FunctionCallOutput { .. },
        ) => {
            call_id != origin
                && name == crate::tools::code_mode::WAIT_TOOL_NAME
                && codex_tools::ToolName::new(namespace.clone(), name).is_default_namespace()
                && serde_json::from_str::<JsonValue>(arguments).is_ok_and(|arguments| {
                    arguments.get("cell_id").and_then(JsonValue::as_str)
                        == Some(runtime_cell.as_str())
                })
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "request_metadata_tests.rs"]
mod tests;
