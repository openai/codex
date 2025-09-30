#![allow(dead_code)]
//! Optional context injection hooks from notifications into live conversations.

#[derive(Clone, Default)]
pub struct ContextInjector;

impl ContextInjector {
    pub fn new() -> Self { Self }
    pub fn inject(&self, _conversation_id: &str, _context: &str) {}
}

