//! Scope: tracks variable bindings across lexical scopes.

use rustc_hash::FxHashMap;

#[derive(Debug, Default)]
pub struct Scope {
    frames: Vec<FxHashMap<String, u32>>,
}

impl Scope {
    pub fn new() -> Self {
        Self { frames: vec![FxHashMap::default()] }
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Enter a new lexical scope.
    pub fn push(&mut self) {
        self.frames.push(FxHashMap::default());
    }

    /// Leave the innermost lexical scope.
    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Define a variable in the current scope.
    pub fn define(&mut self, name: impl Into<String>, slot: u32) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.into(), slot);
        }
    }

    /// Look up a variable, walking outward through scopes.
    pub fn lookup(&self, name: &str) -> Option<u32> {
        for frame in self.frames.iter().rev() {
            if let Some(&slot) = frame.get(name) {
                return Some(slot);
            }
        }
        None
    }
}
