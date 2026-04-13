use codex_thread_store::thread_store_bridge_schema;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema = thread_store_bridge_schema();
    println!("{}", serde_json::to_string_pretty(&schema)?);
    Ok(())
}
