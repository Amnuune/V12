//! JsValue: the host-side representation of a JavaScript value.
//!
//! When the Wasm module calls a host function with an `externref`,
//! Wasmtime gives us a `ExternRef`. We store `JsValue` inside it.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::fmt;

use crate::runtime::HostState;

pub type HostFn = Arc<dyn Fn(&mut wasmtime::Caller<'_, HostState>, JsValue, Vec<JsValue>) -> anyhow::Result<JsValue> + Send + Sync>;

thread_local! {
    pub static GLOBAL_PROTOTYPES: std::cell::RefCell<HashMap<String, JsValue>> = std::cell::RefCell::new(HashMap::new());
    pub static ACTIVE_MEMORY: std::cell::Cell<Option<(*mut u8, usize)>> = std::cell::Cell::new(None);
}

/// A JavaScript value, owned on the Rust host side.
#[derive(Clone)]
pub enum JsValue {
    Undefined,
    Uninitialized, // TDZ sentinel
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    String(Arc<str>),
    Symbol(u64),
    BigInt(i128),
    Object(JsObject),
    Array(JsArray),
    Function(u32),
    HostFunction(HostFn),
}

impl std::fmt::Debug for JsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsValue::Undefined => write!(f, "Undefined"),
            JsValue::Uninitialized => write!(f, "Uninitialized"),
            JsValue::Null => write!(f, "Null"),
            JsValue::Bool(b) => write!(f, "Bool({})", b),
            JsValue::Int(i) => write!(f, "Int({})", i),
            JsValue::Float(fv) => write!(f, "Float({})", fv),
            JsValue::String(s) => write!(f, "String({:?})", s),
            JsValue::Symbol(id) => write!(f, "Symbol({})", id),
            JsValue::BigInt(b) => write!(f, "BigInt({})", b),
            JsValue::Object(obj) => write!(f, "Object({:?})", obj),
            JsValue::Array(arr) => write!(f, "Array({:?})", arr),
            JsValue::Function(idx) => write!(f, "Function({})", idx),
            JsValue::HostFunction(_) => write!(f, "HostFunction"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PropertyDescriptor {
    pub value: Option<JsValue>,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
    pub get: Option<JsValue>,
    pub set: Option<JsValue>,
}

impl Default for PropertyDescriptor {
    fn default() -> Self {
        Self {
            value: None,
            writable: true,
            enumerable: true,
            configurable: true,
            get: None,
            set: None,
        }
    }
}

/// Hidden Class / Object Shape for fast property offset indexing.
#[derive(Debug, Clone, Default)]
pub struct Shape {
    pub property_offsets: HashMap<String, usize>,
}

/// A shared, mutable JS object.
pub type JsObject = Arc<RwLock<JsObjectInner>>;

#[derive(Debug, Default)]
pub struct JsObjectInner {
    pub props: HashMap<String, JsValue>,
    pub descriptors: HashMap<String, PropertyDescriptor>,
    pub property_slots: Vec<JsValue>,
    pub shape: Option<Arc<Shape>>,
    pub proto: Option<JsObject>,
}

/// A shared, mutable JS array.
pub type JsArray = Arc<RwLock<Vec<JsValue>>>;

impl JsValue {
    // ── Tag ──────────────────────────────────────────────────────────────────

    pub fn tag(&self) -> i32 {
        match self {
            JsValue::Undefined | JsValue::Uninitialized  => 0,
            JsValue::Null       => 1,
            JsValue::Bool(_)    => 2,
            JsValue::Int(_)     => 3,
            JsValue::Float(_)   => 4,
            JsValue::String(_)  => 5,
            JsValue::Symbol(_)  => 9,
            JsValue::BigInt(_)  => 10,
            JsValue::Object(_)  => 6,
            JsValue::Array(_)   => 7,
            JsValue::Function(_)=> 8,
            JsValue::HostFunction(_) => 8,
        }
    }

    // ── Truthiness ───────────────────────────────────────────────────────────

    pub fn is_truthy(&self) -> bool {
        match self {
            JsValue::Undefined | JsValue::Uninitialized => false,
            JsValue::Null           => false,
            JsValue::Bool(b)        => *b,
            JsValue::Int(i)         => *i != 0,
            JsValue::Float(f)       => *f != 0.0 && !f.is_nan(),
            JsValue::String(s)      => !s.is_empty(),
            JsValue::Symbol(_)      => true,
            JsValue::BigInt(b)      => *b != 0,
            JsValue::Object(_)      => true,
            JsValue::Array(_)       => true,
            JsValue::Function(_)    => true,
            JsValue::HostFunction(_) => true,
        }
    }

    // ── Coercions ────────────────────────────────────────────────────────────

    pub fn to_number(&self) -> f64 {
        match self {
            JsValue::Undefined | JsValue::Uninitialized => f64::NAN,
            JsValue::Null          => 0.0,
            JsValue::Bool(b)       => if *b { 1.0 } else { 0.0 },
            JsValue::Int(i)        => *i as f64,
            JsValue::Float(f)      => *f,
            JsValue::String(s)     => s.trim().parse::<f64>().unwrap_or(f64::NAN),
            JsValue::Symbol(_)     => f64::NAN,
            JsValue::BigInt(b)     => *b as f64,
            JsValue::Object(_)     => f64::NAN,
            JsValue::Array(a)      => {
                let arr = a.read().unwrap();
                if arr.is_empty() { 0.0 }
                else if arr.len() == 1 { arr[0].to_number() }
                else { f64::NAN }
            }
            JsValue::Function(_)   => f64::NAN,
            JsValue::HostFunction(_) => f64::NAN,
        }
    }

    pub fn to_js_string(&self) -> String {
        match self {
            JsValue::Undefined | JsValue::Uninitialized => "undefined".to_string(),
            JsValue::Null          => "null".to_string(),
            JsValue::Bool(b)       => b.to_string(),
            JsValue::Int(i)        => i.to_string(),
            JsValue::Float(f)      => {
                if f.is_nan()      { "NaN".to_string() }
                else if f.is_infinite() {
                    if *f > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
                } else {
                    // Match JS's number→string behaviour (no trailing zeros, etc.)
                    format_js_float(*f)
                }
            }
            JsValue::String(s)     => s.to_string(),
            JsValue::Symbol(id)    => format!("Symbol({})", id),
            JsValue::BigInt(b)     => format!("{}n", b),
            JsValue::Object(_)     => {
                let name = self.prop_get("name");
                let message = self.prop_get("message");
                if !matches!(name, JsValue::Undefined) {
                    let name_str = name.to_js_string();
                    if !matches!(message, JsValue::Undefined) {
                        let msg_str = message.to_js_string();
                        if !msg_str.is_empty() {
                            format!("{}: {}", name_str, msg_str)
                        } else {
                            name_str
                        }
                    } else {
                        name_str
                    }
                } else {
                    "[object Object]".to_string()
                }
            }
            JsValue::Array(a)      => {
                let arr = a.read().unwrap();
                arr.iter().map(|v| v.to_js_string()).collect::<Vec<_>>().join(",")
            }
            JsValue::Function(_)   => "function() { [native code] }".to_string(),
            JsValue::HostFunction(_) => "function() { [native code] }".to_string(),
        }
    }

    pub fn typeof_str(&self) -> &'static str {
        match self {
            JsValue::Undefined | JsValue::Uninitialized => "undefined",
            JsValue::Null       => "object",   // historic JS quirk
            JsValue::Bool(_)    => "boolean",
            JsValue::Int(_)     => "number",
            JsValue::Float(_)   => "number",
            JsValue::String(_)  => "string",
            JsValue::Symbol(_)  => "symbol",
            JsValue::BigInt(_)  => "bigint",
            JsValue::Object(_)  => "object",
            JsValue::Array(_)   => "object",
            JsValue::Function(_)=> "function",
            JsValue::HostFunction(_)=> "function",
        }
    }

    // ── Arithmetic ───────────────────────────────────────────────────────────

    pub fn js_add(a: &JsValue, b: &JsValue) -> JsValue {
        // If either is a string, concatenate.
        match (a, b) {
            (JsValue::String(s1), _)  => JsValue::String(Arc::from(format!("{}{}", s1, b.to_js_string()))),
            (_, JsValue::String(s2))  => JsValue::String(Arc::from(format!("{}{}", a.to_js_string(), s2))),
            _ => {
                let n = a.to_number() + b.to_number();
                JsValue::from_f64(n)
            }
        }
    }

    pub fn js_sub(a: &JsValue, b: &JsValue) -> JsValue {
        JsValue::from_f64(a.to_number() - b.to_number())
    }
    pub fn js_mul(a: &JsValue, b: &JsValue) -> JsValue {
        JsValue::from_f64(a.to_number() * b.to_number())
    }
    pub fn js_div(a: &JsValue, b: &JsValue) -> JsValue {
        JsValue::from_f64(a.to_number() / b.to_number())
    }
    pub fn js_rem(a: &JsValue, b: &JsValue) -> JsValue {
        JsValue::from_f64(a.to_number() % b.to_number())
    }
    pub fn js_pow(a: &JsValue, b: &JsValue) -> JsValue {
        JsValue::from_f64(a.to_number().powf(b.to_number()))
    }
    pub fn js_neg(a: &JsValue) -> JsValue {
        JsValue::from_f64(-a.to_number())
    }

    // ── Comparison ───────────────────────────────────────────────────────────

    pub fn js_strict_eq(a: &JsValue, b: &JsValue) -> bool {
        match (a, b) {
            (JsValue::Undefined, JsValue::Undefined) => true,
            (JsValue::Null,      JsValue::Null)      => true,
            (JsValue::Bool(x),   JsValue::Bool(y))   => x == y,
            (JsValue::Int(x),    JsValue::Int(y))     => x == y,
            (JsValue::Float(x),  JsValue::Float(y))   => x == y,
            (JsValue::Int(x),    JsValue::Float(y))   => (*x as f64) == *y,
            (JsValue::Float(x),  JsValue::Int(y))     => *x == (*y as f64),
            (JsValue::String(x), JsValue::String(y))  => x == y,
            (JsValue::Symbol(x), JsValue::Symbol(y))  => x == y,
            (JsValue::BigInt(x), JsValue::BigInt(y))  => x == y,
            // Object identity.
            (JsValue::Object(a), JsValue::Object(b)) => Arc::ptr_eq(a, b),
            (JsValue::Array(a),  JsValue::Array(b))  => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn js_abstract_eq(a: &JsValue, b: &JsValue) -> bool {
        // Simplified abstract equality (ES 7.2.14).
        if JsValue::js_strict_eq(a, b) { return true; }
        match (a, b) {
            (JsValue::Null, JsValue::Undefined) | (JsValue::Undefined, JsValue::Null) => true,
            (JsValue::Int(_) | JsValue::Float(_), JsValue::String(_)) => {
                a.to_number() == b.to_number()
            }
            (JsValue::String(_), JsValue::Int(_) | JsValue::Float(_)) => {
                a.to_number() == b.to_number()
            }
            _ => false,
        }
    }

    // ── Bitwise ──────────────────────────────────────────────────────────────

    pub fn to_i32(&self) -> i32 { self.to_number() as i32 }
    pub fn to_u32(&self) -> u32 { self.to_number() as u32 }

    // ── Utilities ────────────────────────────────────────────────────────────

    pub fn from_f64(f: f64) -> JsValue {
        if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 && !f.is_nan() {
            JsValue::Int(f as i32)
        } else {
            JsValue::Float(f)
        }
    }

    pub fn prop_get(&self, key: &str) -> JsValue {
        match self {
            JsValue::Object(obj) => {
                let (ptr, len, size, is_float) = {
                    let guard = obj.read().unwrap();
                    let ptr = guard.props.get("__ptr__").and_then(|v| match v {
                        JsValue::Int(i) => Some(*i as usize),
                        _ => None,
                    });
                    let size = guard.props.get("__element_size__").and_then(|v| match v {
                        JsValue::Int(i) => Some(*i as usize),
                        _ => None,
                    });
                    let is_float = guard.props.get("__is_float__").and_then(|v| match v {
                        JsValue::Bool(b) => Some(*b),
                        _ => None,
                    });
                    (ptr, guard.props.get("length").cloned(), size, is_float)
                };

                if let (Some(base_addr), Some(JsValue::Int(length)), Some(elem_size), Some(float_flag)) = (ptr, len, size, is_float) {
                    if let Ok(idx) = key.parse::<usize>() {
                        if idx < length as usize {
                            let val = ACTIVE_MEMORY.with(|cell| {
                                if let Some((mem_ptr, mem_len)) = cell.get() {
                                    let offset = base_addr + idx * elem_size;
                                    if offset + elem_size <= mem_len {
                                        unsafe {
                                            let ptr = mem_ptr.add(offset);
                                            match (elem_size, float_flag) {
                                                (1, false) => Some(JsValue::Int(*ptr as i32)),
                                                (2, false) => Some(JsValue::Int(*(ptr as *const u16) as i32)),
                                                (4, false) => Some(JsValue::Int(*(ptr as *const i32))),
                                                (4, true) => Some(JsValue::Float(*(ptr as *const f32) as f64)),
                                                (8, false) => Some(JsValue::BigInt(*(ptr as *const i64) as i128)),
                                                (8, true) => Some(JsValue::Float(*(ptr as *const f64))),
                                                _ => None,
                                            }
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            });
                            if let Some(v) = val {
                                return v;
                            }
                        }
                    }
                }

                let mut current = Some(obj.clone());
                while let Some(curr_obj) = current {
                    let guard = curr_obj.read().unwrap();
                    if let Some(val) = guard.props.get(key) {
                        return val.clone();
                    }
                    current = guard.proto.clone();
                }
                // Fallback to Object.prototype
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("Object") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            JsValue::Array(arr) => {
                if key == "length" {
                    let g = arr.read().unwrap();
                    return JsValue::Int(g.len() as i32);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    let g = arr.read().unwrap();
                    if let Some(val) = g.get(idx) {
                        return val.clone();
                    }
                }
                // Fallback to Array.prototype
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("Array") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            JsValue::String(s) => {
                if key == "length" {
                    return JsValue::Int(s.chars().count() as i32);
                }
                if let Ok(idx) = key.parse::<usize>() {
                    let chars: Vec<char> = s.chars().collect();
                    if idx < chars.len() {
                        return JsValue::String(chars[idx].to_string().into());
                    }
                }
                // Fallback to String.prototype
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("String") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            JsValue::Function(_) | JsValue::HostFunction(_) => {
                // Fallback to Function.prototype
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("Function") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            JsValue::Int(_) | JsValue::Float(_) => {
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("Number") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            JsValue::Bool(_) => {
                GLOBAL_PROTOTYPES.with(|p| {
                    if let Some(JsValue::Object(proto)) = p.borrow().get("Boolean") {
                        let guard = proto.read().unwrap();
                        guard.props.get(key).cloned().unwrap_or(JsValue::Undefined)
                    } else {
                        JsValue::Undefined
                    }
                })
            }
            _ => JsValue::Undefined,
        }
    }

    pub fn prop_set(&self, key: &str, val: JsValue) {
        match self {
            JsValue::Object(obj) => {
                let (ptr, len, size, is_float) = {
                    let guard = obj.read().unwrap();
                    let ptr = guard.props.get("__ptr__").and_then(|v| match v {
                        JsValue::Int(i) => Some(*i as usize),
                        _ => None,
                    });
                    let size = guard.props.get("__element_size__").and_then(|v| match v {
                        JsValue::Int(i) => Some(*i as usize),
                        _ => None,
                    });
                    let is_float = guard.props.get("__is_float__").and_then(|v| match v {
                        JsValue::Bool(b) => Some(*b),
                        _ => None,
                    });
                    (ptr, guard.props.get("length").cloned(), size, is_float)
                };

                if let (Some(base_addr), Some(JsValue::Int(length)), Some(elem_size), Some(float_flag)) = (ptr, len, size, is_float) {
                    if let Ok(idx) = key.parse::<usize>() {
                        if idx < length as usize {
                            ACTIVE_MEMORY.with(|cell| {
                                if let Some((mem_ptr, mem_len)) = cell.get() {
                                    let offset = base_addr + idx * elem_size;
                                    if offset + elem_size <= mem_len {
                                        unsafe {
                                            let ptr = mem_ptr.add(offset);
                                            match (elem_size, float_flag) {
                                                (1, false) => {
                                                    let v = match val {
                                                        JsValue::Int(i) => i as u8,
                                                        JsValue::Float(f) => f as u8,
                                                        _ => 0,
                                                    };
                                                    *ptr = v;
                                                }
                                                (2, false) => {
                                                    let v = match val {
                                                        JsValue::Int(i) => i as u16,
                                                        JsValue::Float(f) => f as u16,
                                                        _ => 0,
                                                    };
                                                    *(ptr as *mut u16) = v;
                                                }
                                                (4, false) => {
                                                    let v = match val {
                                                        JsValue::Int(i) => i,
                                                        JsValue::Float(f) => f as i32,
                                                        _ => 0,
                                                    };
                                                    *(ptr as *mut i32) = v;
                                                }
                                                (4, true) => {
                                                    let v = match val {
                                                        JsValue::Float(f) => f as f32,
                                                        JsValue::Int(i) => i as f32,
                                                        _ => 0.0,
                                                    };
                                                    *(ptr as *mut f32) = v;
                                                }
                                                (8, false) => {
                                                    let v = match val {
                                                        JsValue::BigInt(b) => b as i64,
                                                        JsValue::Int(i) => i as i64,
                                                        JsValue::Float(f) => f as i64,
                                                        _ => 0,
                                                    };
                                                    *(ptr as *mut i64) = v;
                                                }
                                                (8, true) => {
                                                    let v = match val {
                                                        JsValue::Float(f) => f,
                                                        JsValue::Int(i) => i as f64,
                                                        _ => 0.0,
                                                    };
                                                    *(ptr as *mut f64) = v;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            });
                            return;
                        }
                    }
                }

                let mut guard = obj.write().unwrap();
                if let Some(desc) = guard.descriptors.get(key) {
                    if !desc.writable {
                        return;
                    }
                }
                guard.props.insert(key.to_string(), val.clone());
                if !guard.descriptors.contains_key(key) {
                    guard.descriptors.insert(key.to_string(), PropertyDescriptor {
                        value: Some(val),
                        writable: true,
                        enumerable: true,
                        configurable: true,
                        get: None,
                        set: None,
                    });
                } else {
                    let desc = guard.descriptors.get_mut(key).unwrap();
                    desc.value = Some(val);
                }
            }
            JsValue::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    if idx < 100_000 {
                        let mut g = arr.write().unwrap();
                        if idx >= g.len() { g.resize(idx + 1, JsValue::Undefined); }
                        g[idx] = val;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn new_object() -> JsValue {
        JsValue::Object(Arc::new(RwLock::new(JsObjectInner::default())))
    }

    pub fn new_array() -> JsValue {
        JsValue::Array(Arc::new(RwLock::new(vec![])))
    }
}

impl fmt::Display for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_js_string())
    }
}

// ── Number formatting ─────────────────────────────────────────────────────────

fn format_js_float(f: f64) -> String {
    // Simple approach: use Rust's default but trim trailing zeros.
    let s = format!("{}", f);
    s
}
