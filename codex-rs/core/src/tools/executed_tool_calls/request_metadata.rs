//! Attaches host observations to request metadata and applies its byte budget.
//! Final request budgeting runs after the recorder lock is released; it never changes tool execution.

use super::*;

impl ExecutedToolCalls {
    /// Attaches trusted completeness; request budgeting only revokes damaged inventories.
    pub(crate) fn attach_to_prompt(
        &self,
        items: &mut [ResponseItem],
        retry_cache: &mut ExecutedToolCallCache,
    ) {
        if self.attach_pending_to_prompt(items, retry_cache) {
            bound_executed_tool_calls_for_prompt(items);
        }
    }

    fn attach_pending_to_prompt(
        &self,
        items: &mut [ResponseItem],
        retry_cache: &mut ExecutedToolCallCache,
    ) -> bool {
        let mut state = self.lock_state();
        let Some(state) = state.as_mut() else {
            return false;
        };
        // Failed wrappers have no callback; only their bounded bitmap observation survives.
        state.pending_wrapper_origins.clear();
        if state.direct_calls.is_empty()
            && state.output_cells.is_empty()
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
            if state.direct_calls.is_empty()
                && state.output_cells.is_empty()
                && pending_outputs.is_empty()
            {
                break;
            }
            let item = &items[index];
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
                let mut calls = state
                    .direct_calls
                    .remove(call_id)
                    .into_iter()
                    .collect::<Vec<_>>();
                let direct_call_collision = !calls.is_empty();
                let mut call_index_by_id = HashMap::new();
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
                    if output_counts.get(call_id) != Some(&1) || !matches_input {
                        cell.completion = CellCompletion::Incomplete;
                    }
                    let pending_calls = cell.pending_calls.len();
                    for (call_id, call) in cell.pending_calls.drain(..) {
                        if matches!(call.arguments(), ExecutedToolCallArguments::Raw(_)) {
                            call_index_by_id.insert(call_id, calls.len());
                        }
                        calls.push(call);
                    }
                    cell.pending_full_argument_bytes = 0;
                    complete = cell.completion == CellCompletion::Complete
                        && !direct_call_collision
                        && (state.can_prove_wait_completion
                            || matches!(item, ResponseItem::CustomToolCallOutput { .. }));
                    if matches!(
                        cell.completion,
                        CellCompletion::Complete | CellCompletion::Incomplete
                    ) {
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
        if !pending_outputs.is_empty() {
            state
                .retained_calls
                .retain(|key, _| !pending_outputs.contains(key));
        }
        let mut invalid_cells = HashSet::new();
        for index in 0..items.len() {
            let item = &items[index];
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
                }
            }
        }
        for item in items.iter_mut() {
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

        let metadata_bytes = items.iter().fold(0_usize, |bytes, item| {
            bytes.saturating_add(executed_tool_call_metadata_bytes(item))
        });
        if metadata_bytes > MAX_EXECUTED_TOOL_CALL_FULL_ARGUMENT_BYTES_PER_OUTPUT {
            bound_executed_tool_calls_for_prompt_prioritizing_recent(items);
            let retained_before_bounding = std::mem::take(&mut state.retained_calls);
            let mut bounded_outputs = HashSet::new();
            for item in items.iter_mut() {
                let Some(call_id) = output_call_id(item) else {
                    continue;
                };
                let key = (std::mem::discriminant(&*item), call_id.to_string());
                // History already carries unmanaged records; retaining them would append them again.
                let Some(previous) = retained_before_bounding.get(&key) else {
                    continue;
                };
                let metadata = item.executed_tool_call_metadata();
                let unique_output = bounded_outputs.insert(key.clone());
                if !unique_output && let Some(retained) = state.retained_calls.get_mut(&key) {
                    retained.call_index_by_id.clear();
                }
                if let Some(runtime_cell_id) = &previous.runtime_cell_id
                    && metadata
                        .is_none_or(|metadata| !metadata.has_same_tool_calls(&previous.calls))
                    && let Some(cell) = state.cells.get_mut(runtime_cell_id)
                {
                    cell.completion = CellCompletion::Incomplete;
                }
                if let Some(metadata) = metadata
                    && (metadata
                        .executed_tool_calls
                        .as_ref()
                        .is_some_and(|calls| !calls.is_empty())
                        || metadata.tool_calls_complete.is_some())
                {
                    // Metadata-only shedding preserves slots; changed calls or duplicate outputs do not.
                    let call_index_by_id =
                        if unique_output && metadata.has_same_tool_calls(&previous.calls) {
                            previous.call_index_by_id.clone()
                        } else {
                            HashMap::new()
                        };
                    let retained = state.retained_calls.entry(key).or_default();
                    retained.runtime_cell_id = previous.runtime_cell_id.clone();
                    retained.calls = metadata.executed_tool_calls.clone().unwrap_or_default();
                    if let Some(cell_id) = metadata.cell_id.as_ref() {
                        retained.cell_id = Some(cell_id.clone());
                    }
                    retained.complete |= metadata.tool_calls_complete == Some(true);
                    retained.call_index_by_id = call_index_by_id;
                    retained.result_metadata_updated = previous.result_metadata_updated;
                }
            }
        }

        attached
    }
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
