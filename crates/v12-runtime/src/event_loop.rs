//! Tokio-based event loop for V12.
//!
//! Manages:
//!   - Microtask queue (Promise callbacks)
//!   - Macrotask queue (setTimeout, setInterval)

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// A task that can be scheduled for later execution.
pub struct Task {
    pub name:    String,
    pub delay:   Duration,
    pub ready_at: Instant,
}

/// The V12 event loop state.
pub struct EventLoop {
    /// Macrotask queue (setTimeout callbacks — stored as function indices).
    pub macrotasks: VecDeque<(u32, Duration)>,
    /// Microtask queue (Promise .then callbacks).
    pub microtasks: VecDeque<u32>,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            macrotasks: VecDeque::new(),
            microtasks: VecDeque::new(),
        }
    }

    pub fn schedule_timeout(&mut self, func_idx: u32, delay_ms: u64) {
        self.macrotasks.push_back((func_idx, Duration::from_millis(delay_ms)));
    }

    pub fn push_microtask(&mut self, func_idx: u32) {
        self.microtasks.push_back(func_idx);
    }

    pub fn next_microtask(&mut self) -> Option<u32> {
        self.microtasks.pop_front()
    }

    pub fn next_macrotask(&mut self) -> Option<u32> {
        self.macrotasks.pop_front().map(|(idx, _)| idx)
    }

    pub fn is_empty(&self) -> bool {
        self.microtasks.is_empty() && self.macrotasks.is_empty()
    }
}
