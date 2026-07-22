//! Runtime host functions registered with Wasmtime.
//!
//! These are the implementations of every "rt_*" import that the compiled
//! Wasm module calls. They operate on `JsValue`s stored as `ExternRef`s.

use std::sync::{Arc, Mutex};
use wasmtime::{Caller, ExternRef, Rooted, StoreContextMut, Val};
use crate::js_value::{JsObjectInner, JsValue};
use crate::runtime::HostState;

/// Convenience: extract a JsValue from an ExternRef.
pub fn from_ref(val: Option<Rooted<ExternRef>>, store: &mut impl wasmtime::AsContextMut) -> JsValue {
    match val {
        Some(r) => {
            let data = r.data(store).unwrap();
            match data {
                Some(d) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                None    => JsValue::Null,
            }
        }
        None => JsValue::Undefined,
    }
}

/// Convenience: box a JsValue into an ExternRef.
pub fn to_ref(val: JsValue, store: &mut impl wasmtime::AsContextMut) -> Option<Rooted<ExternRef>> {
    Some(ExternRef::new(store, val).unwrap())
}

/// Host state: mutable data shared between all host functions.
pub fn make_state() -> HostState {
    HostState {
        string_table: Vec::new(),
        arg_staging:  Vec::new(),
        func_table:   Vec::new(),
        globals:      std::collections::HashMap::new(),
        source:       String::new(),
        exception:   None,
        call_stack:  Vec::new(),
        memory:      None,
        mem_ptr:     0,
    }
}
