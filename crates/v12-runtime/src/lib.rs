//! v12-runtime: Wasmtime host that runs V12-compiled Wasm modules.
//!
//! Exports:
//!   - `Engine`: the V12 engine (compile + run)
//!   - `JsValue`: the host-side JS value representation
//!   - `Runtime`: the Wasmtime instance + event loop

pub mod engine;
pub mod event_loop;
pub mod js_value;
pub mod runtime;
pub mod rt_functions;
pub mod builtins;

pub use engine::V12Engine;
pub use js_value::JsValue;
pub use runtime::Runtime;
