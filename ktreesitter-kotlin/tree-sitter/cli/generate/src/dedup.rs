pub fn split_state_id_groups<S>(
    states: &[S],
    state_ids_by_group_id: &mut Vec<Vec<usize>>,
    group_ids_by_state_id: &mut [usize],
    start_group_id: usize,
    mut should_split: impl FnMut(&S, &S, &[usize]) -> bool,
) -> bool {
    let mut result = false;

    let mut group_id = start_group_id;
    while group_id < state_ids_by_group_id.len() {
        let state_ids = &state_ids_by_group_id[group_id];
        let mut split_state_ids = Vec::new();

        let mut i = 0;
        while i < state_ids.len() {
            let left_state_id = state_ids[i];
            if split_state_ids.contains(&left_state_id) {
                i += 1;
                continue;
            }

            let left_state = &states[left_state_id];

            // Identify all of the other states in the group that are incompatible with
            // this state.
            let mut j = i + 1;
            while j < state_ids.len() {
                let right_state_id = state_ids[j];
                if split_state_ids.contains(&right_state_id) {
                    j += 1;
                    continue;
                }
                let right_state = &states[right_state_id];

                if should_split(left_state, right_state, group_ids_by_state_id) {
                    split_state_ids.push(right_state_id);
                }

                j += 1;
            }

            i += 1;
        }

        // If any states were removed from the group, add them all as a new group.
        if !split_state_ids.is_empty() {
            result = true;
            state_ids_by_group_id[group_id].retain(|i| !split_state_ids.contains(i));

            let new_group_id = state_ids_by_group_id.len();
            for id in &split_state_ids {
                group_ids_by_state_id[*id] = new_group_id;
            }

            state_ids_by_group_id.push(split_state_ids);
        }

        group_id += 1;
    }

    result
}
