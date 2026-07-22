//! Runtime: the Wasmtime host that runs a compiled V12 Wasm module.
//!
//! Responsibilities:
//!   - Set up the Wasmtime engine and linker.
//!   - Register all "v12rt" host functions.
//!   - Instantiate the compiled module.
//!   - Call __main__ and drive the event loop.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use anyhow::{anyhow, Result};
use wasmtime::{
    Config, Engine as WasmEngine, ExternRef, Linker, Module, Rooted,
    Store, Val,
};

use crate::event_loop::EventLoop;
use crate::js_value::{JsObjectInner, JsValue};

pub struct CallFrame {
    pub this: JsValue,
    pub args: Vec<JsValue>,
    pub is_strict: bool,
}

// ─── Host State ───────────────────────────────────────────────────────────────

/// Data stored inside the Wasmtime `Store<T>`.
pub struct HostState {
    /// String table: index → string value. Populated by the engine before run.
    pub string_table: Vec<String>,
    /// Argument staging buffer for variadic calls.
    pub arg_staging:  Vec<JsValue>,
    /// Function index table (wasm func index → JsValue::Function wrapper).
    pub func_table:   Vec<u32>,
    /// Global variable store.
    pub globals:      HashMap<String, JsValue>,
    /// Source code of the current module (for eval).
    pub source:       String,
    /// Thrown exception registry
    pub exception:    Option<JsValue>,
    /// Call frames for active function scopes
    pub call_stack:   Vec<CallFrame>,
    /// Shared memory segment for TypedArrays linear storage
    pub memory:       Option<wasmtime::Memory>,
    /// Next free memory pointer for linear memory allocations
    pub mem_ptr:      u32,
}

// ─── Runtime ─────────────────────────────────────────────────────────────────

pub struct Runtime {
    engine: WasmEngine,
}

impl Runtime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.wasm_gc(true);
        config.wasm_reference_types(true);
        config.wasm_function_references(true);
        config.wasm_tail_call(true);
        config.wasm_simd(true);
        let engine = WasmEngine::new(&config)?;
        Ok(Self { engine })
    }

    /// Run a compiled Wasm module (as bytes) with the given string table.
    pub fn run(&self, wasm_bytes: &[u8], string_table: Vec<String>) -> Result<JsValue> {
        let mut linker: Linker<HostState> = Linker::new(&self.engine);

        // Register all host functions.
        Self::register_host_functions(&mut linker)?;

        let module = Module::from_binary(&self.engine, wasm_bytes)?;

        crate::builtins::init_builtin_prototypes();

        let mut globals = HashMap::new();
        let global_this_obj = JsValue::new_object();
        globals.insert("globalThis".to_string(), global_this_obj);
        globals.insert("Object".to_string(), crate::builtins::get_object_constructor());
        globals.insert("Math".to_string(), crate::builtins::get_math_object());
        globals.insert("Number".to_string(), crate::builtins::get_number_constructor());
        globals.insert("Map".to_string(), crate::builtins::get_map_constructor());
        globals.insert("Set".to_string(), crate::builtins::get_set_constructor());
        globals.insert("Reflect".to_string(), crate::builtins::get_reflect_object());
        globals.insert("Error".to_string(), crate::builtins::get_error_constructor("Error"));
        globals.insert("TypeError".to_string(), crate::builtins::get_error_constructor("TypeError"));
        globals.insert("ReferenceError".to_string(), crate::builtins::get_error_constructor("ReferenceError"));
        globals.insert("RangeError".to_string(), crate::builtins::get_error_constructor("RangeError"));
        globals.insert("SyntaxError".to_string(), crate::builtins::get_error_constructor("SyntaxError"));
        globals.insert("Intl".to_string(), crate::builtins::get_intl_object());

        globals.insert("ArrayBuffer".to_string(), crate::builtins::get_array_buffer_constructor());
        globals.insert("Int8Array".to_string(), crate::builtins::get_typed_array_constructor("Int8Array", 1, false));
        globals.insert("Uint8Array".to_string(), crate::builtins::get_typed_array_constructor("Uint8Array", 1, false));
        globals.insert("Uint8ClampedArray".to_string(), crate::builtins::get_typed_array_constructor("Uint8ClampedArray", 1, false));
        globals.insert("Int16Array".to_string(), crate::builtins::get_typed_array_constructor("Int16Array", 2, false));
        globals.insert("Uint16Array".to_string(), crate::builtins::get_typed_array_constructor("Uint16Array", 2, false));
        globals.insert("Int32Array".to_string(), crate::builtins::get_typed_array_constructor("Int32Array", 4, false));
        globals.insert("Uint32Array".to_string(), crate::builtins::get_typed_array_constructor("Uint32Array", 4, false));
        globals.insert("Float32Array".to_string(), crate::builtins::get_typed_array_constructor("Float32Array", 4, true));
        globals.insert("Float64Array".to_string(), crate::builtins::get_typed_array_constructor("Float64Array", 8, true));
        globals.insert("BigInt64Array".to_string(), crate::builtins::get_typed_array_constructor("BigInt64Array", 8, false));
        globals.insert("BigUint64Array".to_string(), crate::builtins::get_typed_array_constructor("BigUint64Array", 8, false));

        let state = HostState {
            string_table,
            arg_staging: Vec::new(),
            func_table:  Vec::new(),
            globals,
            source:      String::new(),
            exception:   None,
            call_stack:  Vec::new(),
            memory:      None,
            mem_ptr:     0,
        };

        let mut store = Store::new(&self.engine, state);

        let memory_type = wasmtime::MemoryType::new(1, None);
        let memory = wasmtime::Memory::new(&mut store, memory_type)?;
        linker.define(&mut store, "v12rt", "memory", memory)?;
        store.data_mut().memory = Some(memory);

        let data_ptr = memory.data_mut(&mut store).as_mut_ptr();
        let data_len = memory.data_size(&store);
        crate::js_value::ACTIVE_MEMORY.with(|cell| {
            cell.set(Some((data_ptr, data_len)));
        });

        let instance = linker.instantiate(&mut store, &module)?;

        // Call __main__.
        let main = instance
            .get_func(&mut store, "__main__")
            .ok_or_else(|| anyhow!("No __main__ export found"))?;

        let mut results = vec![Val::null_extern_ref()];
        let run_res = main.call(&mut store, &[], &mut results);
        if let Some(exc) = store.data_mut().exception.take() {
            return Err(anyhow::anyhow!("{}", exc.to_js_string()));
        }
        run_res?;

        // Extract result.
        let ret = match &results[0] {
            Val::ExternRef(Some(r)) => {
                let data = r.data(&store)?;
                match data {
                    Some(d) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                    None    => JsValue::Null,
                }
            }
            _ => JsValue::Undefined,
        };

        Ok(ret)
    }

    pub fn register_host_functions(linker: &mut Linker<HostState>) -> Result<()> {
        // ── Constants ────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_undefined", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_null", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::Null)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_true", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::Bool(true))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_false", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::Bool(false))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_int", |mut caller: wasmtime::Caller<'_, HostState>, v: i32| {
            let r = ExternRef::new(&mut caller, JsValue::Int(v))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_float", |mut caller: wasmtime::Caller<'_, HostState>, v: f64| {
            let r = ExternRef::new(&mut caller, JsValue::Float(v))?;
            Ok(Some(r))
        })?;

        // ── String ───────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_string", |mut caller: wasmtime::Caller<'_, HostState>, idx: i32| {
            let s = caller
                .data()
                .string_table
                .get(idx as usize)
                .cloned()
                .unwrap_or_default();
            let r = ExternRef::new(&mut caller, JsValue::String(s.into()))?;
            Ok(Some(r))
        })?;

        // ── Binary ops ───────────────────────────────────────────────────────
        macro_rules! binop {
            ($name:literal, $fn:expr) => {
                linker.func_wrap("v12rt", $name, |mut caller: wasmtime::Caller<'_, HostState>,
                    a: Option<Rooted<ExternRef>>,
                    b: Option<Rooted<ExternRef>>|
                {
                    let av = get_jsval(&a, &caller);
                    let bv = get_jsval(&b, &caller);
                    let result = $fn(&av, &bv);
                    let r = ExternRef::new(&mut caller, result)?;
                    Ok(Some(r))
                })?;
            };
        }

        binop!("rt_add", JsValue::js_add);
        binop!("rt_sub", JsValue::js_sub);
        binop!("rt_mul", JsValue::js_mul);
        binop!("rt_div", JsValue::js_div);
        binop!("rt_rem", JsValue::js_rem);
        binop!("rt_pow", JsValue::js_pow);

        linker.func_wrap("v12rt", "rt_bit_and", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(av.to_i32() & bv.to_i32()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_bit_or", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(av.to_i32() | bv.to_i32()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_bit_xor", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(av.to_i32() ^ bv.to_i32()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_shl", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(av.to_i32() << (bv.to_u32() & 31)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_shr", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(av.to_i32() >> (bv.to_u32() & 31)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_ushr", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let result = (av.to_u32() >> (bv.to_u32() & 31)) as i32;
            let r = ExternRef::new(&mut caller, JsValue::Int(result))?;
            Ok(Some(r))
        })?;

        // Comparisons
        linker.func_wrap("v12rt", "rt_eq", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(JsValue::js_abstract_eq(&av, &bv)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_strict_eq", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(JsValue::js_strict_eq(&av, &bv)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_ne", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(!JsValue::js_abstract_eq(&av, &bv)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_strict_ne", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(!JsValue::js_strict_eq(&av, &bv)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_lt", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let result = js_compare_lt(&av, &bv);
            let r = ExternRef::new(&mut caller, JsValue::Bool(result))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_le", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(!js_compare_lt(&bv, &av)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_gt", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(js_compare_lt(&bv, &av)))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_ge", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(!js_compare_lt(&av, &bv)))?;
            Ok(Some(r))
        })?;

        // Logical
        linker.func_wrap("v12rt", "rt_and", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let result = if !av.is_truthy() { av } else { bv };
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_or", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let result = if av.is_truthy() { av } else { bv };
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_nullish", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let result = match &av {
                JsValue::Undefined | JsValue::Null => bv,
                _ => av,
            };
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_in", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let bv = get_jsval(&b, &caller);
            let key = av.to_js_string();
            let found = match &bv {
                JsValue::Object(obj) => obj.read().unwrap().props.contains_key(&key),
                JsValue::Array(arr)  => {
                    let g = arr.read().unwrap();
                    key.parse::<usize>().map(|i| i < g.len()).unwrap_or(false)
                }
                _ => false,
            };
            let r = ExternRef::new(&mut caller, JsValue::Bool(found))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_instanceof", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
        {
            // Simplified: always false for now.
            let r = ExternRef::new(&mut caller, JsValue::Bool(false))?;
            Ok(Some(r))
        })?;

        // ── Unary ops ────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_neg", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let r = ExternRef::new(&mut caller, JsValue::js_neg(&av))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_not", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(!av.is_truthy()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_bit_not", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Int(!av.to_i32()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_typeof", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let s: std::sync::Arc<str> = av.typeof_str().into();
            let r = ExternRef::new(&mut caller, JsValue::String(s))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_void", |mut caller: wasmtime::Caller<'_, HostState>,
            _a: Option<Rooted<ExternRef>>|
        {
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_delete", |mut caller: wasmtime::Caller<'_, HostState>,
            _a: Option<Rooted<ExternRef>>|
        {
            let r = ExternRef::new(&mut caller, JsValue::Bool(true))?;
            Ok(Some(r))
        })?;

        // Inc/Dec
        for (name, delta) in [("rt_pre_inc", 1i32), ("rt_pre_dec", -1i32),
                               ("rt_post_inc", 1i32), ("rt_post_dec", -1i32)] {
            let d = delta;
            linker.func_wrap("v12rt", name, move |mut caller: wasmtime::Caller<'_, HostState>,
                a: Option<Rooted<ExternRef>>|
            {
                let av = get_jsval(&a, &caller);
                let n = av.to_number() + d as f64;
                let r = ExternRef::new(&mut caller, JsValue::from_f64(n))?;
                Ok(Some(r))
            })?;
        }

        // ── Type coercions ───────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_truthy", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let r = ExternRef::new(&mut caller, JsValue::Bool(av.is_truthy()))?;
            Ok(Some(r))
        })?;

        // Returns i32 (0 or 1) for Wasm `if` branch conditions.
        linker.func_wrap("v12rt", "rt_truthy_i32", |caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            Ok(if av.is_truthy() { 1i32 } else { 0i32 })
        })?;

        linker.func_wrap("v12rt", "rt_to_number", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let r = ExternRef::new(&mut caller, JsValue::from_f64(av.to_number()))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_to_string", |mut caller: wasmtime::Caller<'_, HostState>,
            a: Option<Rooted<ExternRef>>|
        {
            let av = get_jsval(&a, &caller);
            let s: std::sync::Arc<str> = av.to_js_string().into();
            let r = ExternRef::new(&mut caller, JsValue::String(s))?;
            Ok(Some(r))
        })?;

        // ── Objects / Arrays / Props ─────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_object_new", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::new_object())?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_array_new", |mut caller: wasmtime::Caller<'_, HostState>| {
            let r = ExternRef::new(&mut caller, JsValue::new_array())?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_prop_get", |mut caller: wasmtime::Caller<'_, HostState>,
            obj: Option<Rooted<ExternRef>>, key: Option<Rooted<ExternRef>>|
        {
            let obj_val = get_jsval(&obj, &caller);
            let key_val = get_jsval(&key, &caller);
            let key_str = key_val.to_js_string();
            let result  = obj_val.prop_get(&key_str);
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_prop_set", |mut caller: wasmtime::Caller<'_, HostState>,
            obj: Option<Rooted<ExternRef>>,
            key: Option<Rooted<ExternRef>>,
            val: Option<Rooted<ExternRef>>|
        {
            let obj_val = get_jsval(&obj, &caller);
            let key_val = get_jsval(&key, &caller);
            let val_val = get_jsval(&val, &caller);
            let key_str = key_val.to_js_string();
            obj_val.prop_set(&key_str, val_val);
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_array_get", |mut caller: wasmtime::Caller<'_, HostState>,
            arr: Option<Rooted<ExternRef>>, idx: Option<Rooted<ExternRef>>|
        {
            let arr_val = get_jsval(&arr, &caller);
            let idx_val = get_jsval(&idx, &caller);
            let result  = arr_val.prop_get(&idx_val.to_js_string());
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_array_set", |mut caller: wasmtime::Caller<'_, HostState>,
            arr: Option<Rooted<ExternRef>>,
            idx: Option<Rooted<ExternRef>>,
            val: Option<Rooted<ExternRef>>|
        {
            let arr_val = get_jsval(&arr, &caller);
            let idx_val = get_jsval(&idx, &caller);
            let val_val = get_jsval(&val, &caller);
            arr_val.prop_set(&idx_val.to_js_string(), val_val);
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_object_spread", |mut caller: wasmtime::Caller<'_, HostState>,
            target: Option<Rooted<ExternRef>>, source: Option<Rooted<ExternRef>>|
        {
            let tgt_val = get_jsval(&target, &caller);
            let src_val = get_jsval(&source, &caller);
            if let JsValue::Object(tgt) = &tgt_val {
                match &src_val {
                    JsValue::Object(src) => {
                        let src_guard = src.read().unwrap();
                        let mut tgt_guard = tgt.write().unwrap();
                        for (k, v) in &src_guard.props {
                            tgt_guard.props.insert(k.clone(), v.clone());
                        }
                    }
                    JsValue::String(s) => {
                        let mut tgt_guard = tgt.write().unwrap();
                        for (i, c) in s.chars().enumerate() {
                            tgt_guard.props.insert(i.to_string(), JsValue::String(c.to_string().into()));
                        }
                    }
                    _ => {}
                }
            }
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_array_spread", |mut caller: wasmtime::Caller<'_, HostState>,
            target: Option<Rooted<ExternRef>>, source: Option<Rooted<ExternRef>>|
        {
            let tgt_val = get_jsval(&target, &caller);
            let src_val = get_jsval(&source, &caller);
            if let JsValue::Array(tgt) = &tgt_val {
                match &src_val {
                    JsValue::Array(src) => {
                        let src_guard = src.read().unwrap();
                        let mut tgt_guard = tgt.write().unwrap();
                        for item in src_guard.iter() {
                            tgt_guard.push(item.clone());
                        }
                    }
                    JsValue::String(s) => {
                        let mut tgt_guard = tgt.write().unwrap();
                        for c in s.chars() {
                            tgt_guard.push(JsValue::String(c.to_string().into()));
                        }
                    }
                    _ => {}
                }
            }
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_get_keys", |mut caller: wasmtime::Caller<'_, HostState>,
            obj: Option<Rooted<ExternRef>>|
        {
            let val = get_jsval(&obj, &caller);
            let mut keys = Vec::new();
            match &val {
                JsValue::Object(o) => {
                    let guard = o.read().unwrap();
                    for k in guard.props.keys() {
                        if k != "__proto__" && k != "prototype" {
                            keys.push(JsValue::String(k.clone().into()));
                        }
                    }
                }
                JsValue::Array(a) => {
                    let guard = a.read().unwrap();
                    for i in 0..guard.len() {
                        keys.push(JsValue::String(i.to_string().into()));
                    }
                }
                _ => {}
            }
            let r = ExternRef::new(&mut caller, JsValue::Array(Arc::new(RwLock::new(keys))))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_get_values", |mut caller: wasmtime::Caller<'_, HostState>,
            obj: Option<Rooted<ExternRef>>|
        {
            let val = get_jsval(&obj, &caller);
            let mut values = Vec::new();
            match &val {
                JsValue::Array(a) => {
                    let guard = a.read().unwrap();
                    for item in guard.iter() {
                        values.push(item.clone());
                    }
                }
                JsValue::String(s) => {
                    for c in s.chars() {
                        values.push(JsValue::String(c.to_string().into()));
                    }
                }
                JsValue::Object(o) => {
                    let guard = o.read().unwrap();
                    for v in guard.props.values() {
                        values.push(v.clone());
                    }
                }
                _ => {}
            }
            let r = ExternRef::new(&mut caller, JsValue::Array(Arc::new(RwLock::new(values))))?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_array_push", |mut caller: wasmtime::Caller<'_, HostState>,
            target: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>|
        {
            let tgt_val = get_jsval(&target, &caller);
            let val_val = get_jsval(&value, &caller);
            if let JsValue::Array(tgt) = &tgt_val {
                tgt.write().unwrap().push(val_val);
            }
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        // ── Calls ────────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_push_arg", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            caller.data_mut().arg_staging.push(v);
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_call", |mut caller: wasmtime::Caller<'_, HostState>,
            callee: Option<Rooted<ExternRef>>,
            this:  Option<Rooted<ExternRef>>,
            argc:  i32|
        {
            let callee_val = get_jsval(&callee, &caller);
            let this_val = get_jsval(&this, &caller);
            
            let mut args = Vec::new();
            for _ in 0..argc {
                if let Some(arg) = caller.data_mut().arg_staging.pop() {
                    args.push(arg);
                }
            }
            args.reverse();

            let result = match &callee_val {
                JsValue::Function(idx) => {
                    let u_idx = *idx;
                    invoke_wasm_func(&mut caller, u_idx, this_val, args)
                }
                JsValue::Int(idx) => {
                    let u_idx = *idx as u32;
                    invoke_wasm_func(&mut caller, u_idx, this_val, args)
                }
                JsValue::HostFunction(f) => {
                    f(&mut caller, this_val, args).unwrap_or(JsValue::Undefined)
                }
                _ => JsValue::Undefined,
            };
            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_new", |mut caller: wasmtime::Caller<'_, HostState>,
            _ctor: Option<Rooted<ExternRef>>|
        {
            let r = ExternRef::new(&mut caller, JsValue::new_object())?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_js_new", |mut caller: wasmtime::Caller<'_, HostState>,
            ctor: Option<Rooted<ExternRef>>,
            _this: Option<Rooted<ExternRef>>,
            argc: i32|
        {
            let ctor_val = get_jsval(&ctor, &caller);
            let proto = ctor_val.prop_get("prototype");
            
            let new_obj = JsValue::new_object();
            if let JsValue::Object(new_inner) = &new_obj {
                if let JsValue::Object(proto_inner) = proto {
                    new_inner.write().unwrap().proto = Some(proto_inner);
                }
            }

            let mut args = Vec::new();
            for _ in 0..argc {
                if let Some(arg) = caller.data_mut().arg_staging.pop() {
                    args.push(arg);
                }
            }
            args.reverse();

            let result = match &ctor_val {
                JsValue::Function(idx) => {
                    let u_idx = *idx;
                    let ctor_ret = invoke_wasm_func(&mut caller, u_idx, new_obj.clone(), args);
                    match ctor_ret {
                        JsValue::Object(_) | JsValue::Array(_) => ctor_ret,
                        _ => new_obj,
                    }
                }
                JsValue::Int(idx) => {
                    let u_idx = *idx as u32;
                    let ctor_ret = invoke_wasm_func(&mut caller, u_idx, new_obj.clone(), args);
                    match ctor_ret {
                        JsValue::Object(_) | JsValue::Array(_) => ctor_ret,
                        _ => new_obj,
                    }
                }
                JsValue::HostFunction(f) => {
                    let ctor_ret = f(&mut caller, new_obj.clone(), args).unwrap_or(JsValue::Undefined);
                    match ctor_ret {
                        JsValue::Object(_) | JsValue::Array(_) => ctor_ret,
                        _ => new_obj,
                    }
                }
                _ => new_obj,
            };

            let r = ExternRef::new(&mut caller, result)?;
            Ok(Some(r))
        })?;

        // ── Console ──────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_console_log", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            println!("{}", v.to_js_string());
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_console_error", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            eprintln!("{}", v.to_js_string());
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_console_warn", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            eprintln!("[WARN] {}", v.to_js_string());
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        // ── Globals ──────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_global_get", |mut caller: wasmtime::Caller<'_, HostState>,
            key: Option<Rooted<ExternRef>>|
        {
            let k = get_jsval(&key, &caller);
            let key_str = k.to_js_string();
            let val = caller.data().globals.get(&key_str).cloned().unwrap_or(JsValue::Undefined);
            if matches!(val, JsValue::Uninitialized) {
                let exc = JsValue::new_object();
                exc.prop_set("name", JsValue::String("ReferenceError".into()));
                exc.prop_set("message", JsValue::String(format!("Cannot access '{}' before initialization", key_str).into()));
                caller.data_mut().exception = Some(exc);
                let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                return Ok(Some(r));
            }
            let r = ExternRef::new(&mut caller, val)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_global_set", |mut caller: wasmtime::Caller<'_, HostState>,
            key: Option<Rooted<ExternRef>>,
            val: Option<Rooted<ExternRef>>|
        {
            let k = get_jsval(&key, &caller);
            let v = get_jsval(&val, &caller);
            let key_str = k.to_js_string();
            caller.data_mut().globals.insert(key_str, v);
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        // ── eval() ───────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_eval_js", |mut caller: wasmtime::Caller<'_, HostState>,
            code: Option<Rooted<ExternRef>>|
        {
            let code_val = get_jsval(&code, &caller);
            let code_str = code_val.to_js_string();
            log::warn!("eval() called with: {}", &code_str[..code_str.len().min(80)]);

            let parsed = match v12_frontend::ParsedProgram::from_source(&code_str) {
                Ok(p) => p,
                Err(e) => {
                    caller.data_mut().exception = Some(JsValue::String(format!("SyntaxError: {}", e).into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };
            let program = parsed.program();
            let lifter = v12_lifter::Lifter::new();
            let ir_module = match lifter.lift_program(&program) {
                Ok(m) => m,
                Err(e) => {
                    caller.data_mut().exception = Some(JsValue::String(format!("CompileError: {}", e).into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };

            let offset = caller.data().string_table.len() as u32;
            let mut new_strings = ir_module.strings.clone();
            caller.data_mut().string_table.append(&mut new_strings);

            let emitter = v12_emitter::Emitter::with_offset(&ir_module, offset);
            let wasm_bytes = match emitter.emit() {
                Ok(b) => b,
                Err(e) => {
                    caller.data_mut().exception = Some(JsValue::String(format!("EmitError: {}", e).into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };

            let engine = caller.engine().clone();
            let module = match wasmtime::Module::from_binary(&engine, &wasm_bytes) {
                Ok(m) => m,
                Err(e) => {
                    caller.data_mut().exception = Some(JsValue::String(format!("LoadError: {}", e).into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };

            let mut dynamic_linker = Linker::new(&engine);
            let memory = caller.data().memory.unwrap();
            if let Err(e) = dynamic_linker.define(&mut caller, "v12rt", "memory", memory) {
                caller.data_mut().exception = Some(JsValue::String(format!("LinkerError: {}", e).into()));
                let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                return Ok(Some(r));
            }
            if let Err(e) = Self::register_host_functions(&mut dynamic_linker) {
                caller.data_mut().exception = Some(JsValue::String(format!("LinkerError: {}", e).into()));
                let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                return Ok(Some(r));
            }

            let instance = match dynamic_linker.instantiate(&mut caller, &module) {
                Ok(inst) => inst,
                Err(e) => {
                    caller.data_mut().exception = Some(JsValue::String(format!("InstantiationError: {}", e).into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };

            let main = match instance.get_func(&mut caller, "__main__") {
                Some(f) => f,
                None => {
                    caller.data_mut().exception = Some(JsValue::String("No __main__ export found".to_string().into()));
                    let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                    return Ok(Some(r));
                }
            };

            let mut results = vec![Val::null_extern_ref()];
            if let Err(e) = main.call(&mut caller, &[], &mut results) {
                caller.data_mut().exception = Some(JsValue::String(format!("ExecutionError: {}", e).into()));
                let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
                return Ok(Some(r));
            }

            let ret = match &results[0] {
                Val::ExternRef(Some(r)) => {
                    let data = r.data(&caller)?;
                    match data {
                        Some(d) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                        None    => JsValue::Null,
                    }
                }
                _ => JsValue::Undefined,
            };

            let r = ExternRef::new(&mut caller, ret)?;
            Ok(Some(r))
        })?;

        // ── tail_call_frame() ─────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_tail_call_frame", |mut caller: wasmtime::Caller<'_, HostState>,
            this: Option<Rooted<ExternRef>>,
            argc: i32|
        {
            let this_val = get_jsval(&this, &caller);
            let mut args = Vec::new();
            for _ in 0..argc {
                if let Some(arg) = caller.data_mut().arg_staging.pop() {
                    args.push(arg);
                }
            }
            args.reverse();
            caller.data_mut().call_stack.pop();
            caller.data_mut().call_stack.push(CallFrame {
                this: this_val,
                args,
                is_strict: false,
            });
            Ok(())
        })?;

        // ── Async ────────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_js_await", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            let r = ExternRef::new(&mut caller, v)?;
            Ok(Some(r))
        })?;

        // ── Exceptions ───────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_has_exception", |caller: wasmtime::Caller<'_, HostState>| {
            Ok(if caller.data().exception.is_some() { 1i32 } else { 0i32 })
        })?;

        linker.func_wrap("v12rt", "rt_get_exception", |mut caller: wasmtime::Caller<'_, HostState>| {
            let exc = caller.data_mut().exception.take().unwrap_or(JsValue::Undefined);
            let r = ExternRef::new(&mut caller, exc)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_get_arg", |mut caller: wasmtime::Caller<'_, HostState>, idx: i32| {
            let val = caller.data().call_stack.last()
                .and_then(|frame| frame.args.get(idx as usize).cloned())
                .unwrap_or(JsValue::Undefined);
            let r = ExternRef::new(&mut caller, val)?;
            Ok(Some(r))
        })?;

        linker.func_wrap("v12rt", "rt_get_this", |mut caller: wasmtime::Caller<'_, HostState>| {
            let val = caller.data().call_stack.last()
                .map(|frame| frame.this.clone())
                .unwrap_or(JsValue::Undefined);
            let r = ExternRef::new(&mut caller, val)?;
            Ok(Some(r))
        })?;

        // ── Throw ────────────────────────────────────────────────────────────
        linker.func_wrap("v12rt", "rt_throw", |mut caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>|
        {
            let v = get_jsval(&val, &caller);
            caller.data_mut().exception = Some(v);
            let r = ExternRef::new(&mut caller, JsValue::Undefined)?;
            Ok(Some(r))
        })?;

        // ── TypedArray memory helpers ──
        linker.func_wrap("v12rt", "rt_typed_array_ptr", |caller: wasmtime::Caller<'_, HostState>,
            obj: Option<Rooted<ExternRef>>| -> Result<i32>
        {
            let val = get_jsval(&obj, &caller);
            if let JsValue::Object(o) = val {
                let guard = o.read().unwrap();
                if let Some(JsValue::Int(ptr)) = guard.props.get("__ptr__") {
                    return Ok(*ptr);
                }
            }
            Ok(0)
        })?;

        linker.func_wrap("v12rt", "rt_to_int32", |caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>| -> Result<i32>
        {
            let v = get_jsval(&val, &caller);
            match v {
                JsValue::Int(i) => Ok(i),
                JsValue::Float(f) => Ok(f as i32),
                _ => Ok(0),
            }
        })?;

        linker.func_wrap("v12rt", "rt_val_to_i32", |caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>| -> Result<i32>
        {
            let v = get_jsval(&val, &caller);
            match v {
                JsValue::Int(i) => Ok(i),
                JsValue::Float(f) => Ok(f as i32),
                _ => Ok(0),
            }
        })?;

        linker.func_wrap("v12rt", "rt_val_to_f64", |caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>| -> Result<f64>
        {
            let v = get_jsval(&val, &caller);
            match v {
                JsValue::Float(f) => Ok(f),
                JsValue::Int(i) => Ok(i as f64),
                _ => Ok(0.0),
            }
        })?;

        linker.func_wrap("v12rt", "rt_val_to_i64", |caller: wasmtime::Caller<'_, HostState>,
            val: Option<Rooted<ExternRef>>| -> Result<i64>
        {
            let v = get_jsval(&val, &caller);
            match v {
                JsValue::Int(i) => Ok(i as i64),
                JsValue::Float(f) => Ok(f as i64),
                JsValue::BigInt(b) => Ok(b as i64),
                _ => Ok(0),
            }
        })?;

        linker.func_wrap("v12rt", "rt_bigint_from_i64", |mut caller: wasmtime::Caller<'_, HostState>,
            val: i64| -> Result<Option<Rooted<ExternRef>>>
        {
            let r = ExternRef::new(&mut caller, JsValue::BigInt(val as i128))?;
            Ok(Some(r))
        })?;

        Ok(())
    }
}

// ─── Helper: extract JsValue from ExternRef ───────────────────────────────────

fn get_jsval(r: &Option<Rooted<ExternRef>>, store: &impl wasmtime::AsContext) -> JsValue {
    match r {
        Some(ref r) => {
            match r.data(store) {
                Ok(Some(d)) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                _ => JsValue::Undefined,
            }
        }
        None => JsValue::Undefined,
    }
}

fn js_compare_lt(a: &JsValue, b: &JsValue) -> bool {
    match (a, b) {
        (JsValue::String(s1), JsValue::String(s2)) => s1.as_ref() < s2.as_ref(),
        _ => a.to_number() < b.to_number(),
    }
}

fn invoke_wasm_func(
    caller: &mut wasmtime::Caller<'_, HostState>,
    idx: u32,
    this_val: JsValue,
    args: Vec<JsValue>,
) -> JsValue {
    if let Some(wasmtime::Extern::Func(func)) = caller.get_export(&format!("func_{}", idx)) {
        caller.data_mut().call_stack.push(CallFrame {
            this: this_val,
            args,
            is_strict: false,
        });
        
        let mut results = vec![wasmtime::Val::null_extern_ref()];
        let run_res = func.call(&mut *caller, &[], &mut results);
        
        caller.data_mut().call_stack.pop();
        
        match run_res {
            Ok(_) => {
                match &results[0] {
                    wasmtime::Val::ExternRef(Some(r)) => {
                        let data = r.data(&*caller).unwrap_or(None);
                        match data {
                            Some(d) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                            None => JsValue::Undefined,
                        }
                    }
                    _ => JsValue::Undefined,
                }
            }
            Err(_) => JsValue::Undefined,
        }
    } else {
        JsValue::Undefined
    }
}
