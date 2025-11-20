use rand::rngs::SmallRng;
use rand::RngCore;
use rand::SeedableRng;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CapSids {
    pub workspace: String,
    pub readonly: String,
}

pub fn cap_sid_file(codex_home: &Path) -> PathBuf {
    codex_home.join("cap_sid")
}

fn make_random_cap_sid_string() -> String {
    let mut rng = SmallRng::from_entropy();
    let a = rng.next_u32();
    let b = rng.next_u32();
    let c = rng.next_u32();
    let d = rng.next_u32();
    format!("S-1-5-21-{}-{}-{}-{}", a, b, c, d)
}

pub fn load_or_create_cap_sids(codex_home: &Path) -> CapSids {
    let path = cap_sid_file(codex_home);
    if path.exists() {
        if let Ok(txt) = fs::read_to_string(&path) {
            let t = txt.trim();
            if t.starts_with('{') && t.ends_with('}') {
                if let Ok(obj) = serde_json::from_str::<CapSids>(t) {
                    return obj;
                }
            } else if !t.is_empty() {
                return CapSids {
                    workspace: t.to_string(),
                    readonly: make_random_cap_sid_string(),
                };
            }
        }
    }
    CapSids {
        workspace: make_random_cap_sid_string(),
        readonly: make_random_cap_sid_string(),
    }
}
