#![cfg(all(feature = "managed", feature = "serde"))]

use std::{collections::HashMap, env, time::Duration};

use config::Config;
use serde::{Deserialize, Serialize};

use deadpool::managed::PoolConfig;

struct Env {
    backup: HashMap<String, Option<String>>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            backup: HashMap::new(),
        }
    }
    pub fn set(&mut self, name: &str, value: &str) {
        self.backup.insert(name.to_string(), env::var(name).ok());
        env::set_var(name, value);
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        for (name, value) in self.backup.iter() {
            match value {
                Some(value) => env::set_var(name.as_str(), value),
                None => env::remove_var(name.as_str()),
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TestConfig {
    pool: PoolConfig,
}

#[test]
fn from_env() {
    let mut env = Env::new();
    env.set("POOL__MAX_SIZE", "42");
    env.set("POOL__TIMEOUTS__WAIT__SECS", "1");
    env.set("POOL__TIMEOUTS__WAIT__NANOS", "0");
    env.set("POOL__TIMEOUTS__CREATE__SECS", "2");
    env.set("POOL__TIMEOUTS__CREATE__NANOS", "0");
    env.set("POOL__TIMEOUTS__RECYCLE__SECS", "3");
    env.set("POOL__TIMEOUTS__RECYCLE__NANOS", "0");

    let cfg = Config::builder()
        .add_source(config::Environment::default().separator("__"))
        .build()
        .unwrap()
        .try_deserialize::<TestConfig>()
        .unwrap();

    assert_eq!(cfg.pool.max_size, 42);
    assert_eq!(cfg.pool.timeouts.wait, Some(Duration::from_secs(1)));
    assert_eq!(cfg.pool.timeouts.create, Some(Duration::from_secs(2)));
    assert_eq!(cfg.pool.timeouts.recycle, Some(Duration::from_secs(3)));
}
