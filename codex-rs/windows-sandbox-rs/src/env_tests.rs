use super::prepend_path;
use super::reorder_pathext_for_stubs;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

#[test]
fn path_mutations_are_case_insensitive_and_do_not_fall_back_to_parent_values() {
    let mut env = HashMap::from([
        ("Path".to_string(), r"C:\child".to_string()),
        ("PathExt".to_string(), ".EXE;.CMD".to_string()),
    ]);

    prepend_path(&mut env, r"C:\deny");
    reorder_pathext_for_stubs(&mut env);

    assert_eq!(
        env,
        HashMap::from([
            ("PATH".to_string(), r"C:\deny;C:\child".to_string()),
            ("PATHEXT".to_string(), ".CMD;.EXE".to_string()),
        ])
    );
}
