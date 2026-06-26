pub(crate) fn scroll_expression(delta_x: i64, delta_y: i64) -> String {
    format!("(() => {{ window.scrollBy({delta_x}, {delta_y}); return {{ scrollX, scrollY }}; }})()")
}

pub(crate) fn key_code(key: &str) -> &str {
    match key {
        "Enter" => "Enter",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "Backspace",
        "Delete" => "Delete",
        "ArrowUp" => "ArrowUp",
        "ArrowDown" => "ArrowDown",
        "ArrowLeft" => "ArrowLeft",
        "ArrowRight" => "ArrowRight",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PageUp",
        "PageDown" => "PageDown",
        _ => key,
    }
}
