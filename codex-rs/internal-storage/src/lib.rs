use serde_json::Map as JsonMap;
use serde_json::Value;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use thiserror::Error;

const INTERNAL_STORAGE_FILENAME: &str = "internal_storage.json";

#[derive(Debug, Error)]
pub enum InternalStorageError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("internal storage has not been initialized")]
    Uninitialized,
}

#[derive(Debug)]
struct Storage {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Storage {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn read_map(&self) -> Result<JsonMap<String, Value>, InternalStorageError> {
        match fs::read_to_string(&self.path) {
            Ok(contents) => {
                let value: Value = serde_json::from_str(&contents)?;
                match value {
                    Value::Object(map) => Ok(map),
                    Value::Null => Ok(JsonMap::new()),
                    _ => Ok(JsonMap::new()),
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(JsonMap::new()),
            Err(err) => Err(err.into()),
        }
    }

    fn write_map(&self, map: &JsonMap<String, Value>) -> Result<(), InternalStorageError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_string_pretty(&Value::Object(map.clone()))?;
        fs::write(&self.path, payload)?;
        Ok(())
    }

    fn read(&self, key: &str) -> Result<Option<String>, InternalStorageError> {
        let _guard = self.lock.lock().expect("internal storage lock poisoned");
        let map = self.read_map()?;
        Ok(map.get(key).map(value_to_string))
    }

    fn write(&self, key: &str, value: &str) -> Result<(), InternalStorageError> {
        let _guard = self.lock.lock().expect("internal storage lock poisoned");
        let mut map = self.read_map()?;
        map.insert(key.to_string(), Value::String(value.to_string()));
        self.write_map(&map)
    }
}

static STORAGE: OnceLock<Storage> = OnceLock::new();

pub fn initialize(codex_home: PathBuf) {
    let path = build_storage_path(&codex_home);
    let storage = Storage::new(path.clone());
    match STORAGE.get() {
        Some(existing) if existing.path != path => {}
        Some(_) => {}
        None => {
            let _ = STORAGE.set(storage);
        }
    }
}

pub fn read(key: &str) -> Result<Option<String>, InternalStorageError> {
    storage()?.read(key)
}

pub fn write(key: &str, value: &str) -> Result<(), InternalStorageError> {
    storage()?.write(key, value)
}

fn storage() -> Result<&'static Storage, InternalStorageError> {
    STORAGE.get().ok_or(InternalStorageError::Uninitialized)
}

fn build_storage_path(codex_home: &Path) -> PathBuf {
    codex_home.join(INTERNAL_STORAGE_FILENAME)
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}
