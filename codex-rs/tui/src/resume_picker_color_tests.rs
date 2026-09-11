use super::*;

#[test]
fn thread_colors_survive_selection_in_both_picker_layouts() {
    let id = ThreadId::from_u128(/*value*/ 42);
    let row = Row {
        path: None,
        thread_id: Some(id),
        thread_name: Some("Named task".into()),
        preview: "Original prompt".into(),
        created_at: None,
        updated_at: None,
        cwd: None,
        git_branch: None,
    };
    let mut state = PickerState::new(
        FrameRequester::test_dummy(),
        Arc::new(|_| {}),
        ProviderFilter::Any,
        /*show_all*/ true,
        /*filter_cwd*/ None,
        SessionPickerAction::Resume,
    );
    let mut snapshot = Vec::new();
    for density in [SessionListDensity::Comfortable, SessionListDensity::Dense] {
        state.density = density;
        for use_colors in [true, false] {
            state.use_theme_colors = use_colors;
            for selected in [true, false] {
                let lines = render_session_lines(
                    &row, &state, selected, /*is_expanded*/ false, /*is_zebra*/ false,
                    /*width*/ 32,
                );
                snapshot.push(format!(
                    "{density:?} colors={use_colors} selected={selected}: {:?}",
                    lines[0]
                ));
            }
        }
    }
    insta::assert_snapshot!(snapshot.join("\n"));
}
