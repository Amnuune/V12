use crate::js_value::{JsValue, HostFn, PropertyDescriptor, GLOBAL_PROTOTYPES};
use crate::runtime::{HostState, CallFrame};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub fn init_builtin_prototypes() {
    GLOBAL_PROTOTYPES.with(|p| {
        let mut map = p.borrow_mut();
        if map.contains_key("Object") {
            return; // Already initialized
        }

        // 1. Object.prototype
        let obj_proto = JsValue::new_object();
        obj_proto.prop_set("toString", JsValue::HostFunction(Arc::new(object_to_string)));
        map.insert("Object".to_string(), obj_proto.clone());

        // 2. String.prototype
        let str_proto = JsValue::new_object();
        str_proto.prop_set("indexOf", JsValue::HostFunction(Arc::new(string_index_of)));
        str_proto.prop_set("slice", JsValue::HostFunction(Arc::new(string_slice)));
        str_proto.prop_set("charAt", JsValue::HostFunction(Arc::new(string_char_at)));
        str_proto.prop_set("replace", JsValue::HostFunction(Arc::new(string_replace)));
        str_proto.prop_set("replaceAll", JsValue::HostFunction(Arc::new(string_replace_all)));
        str_proto.prop_set("split", JsValue::HostFunction(Arc::new(string_split)));
        str_proto.prop_set("trim", JsValue::HostFunction(Arc::new(string_trim)));
        str_proto.prop_set("startsWith", JsValue::HostFunction(Arc::new(string_starts_with)));
        str_proto.prop_set("endsWith", JsValue::HostFunction(Arc::new(string_ends_with)));
        str_proto.prop_set("includes", JsValue::HostFunction(Arc::new(string_includes)));
        str_proto.prop_set("padStart", JsValue::HostFunction(Arc::new(string_pad_start)));
        str_proto.prop_set("padEnd", JsValue::HostFunction(Arc::new(string_pad_end)));
        str_proto.prop_set("bold", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<b>{}</b>", this.to_js_string()).into())))));
        str_proto.prop_set("italics", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<i>{}</i>", this.to_js_string()).into())))));
        str_proto.prop_set("strike", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<strike>{}</strike>", this.to_js_string()).into())))));
        str_proto.prop_set("sub", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<sub>{}</sub>", this.to_js_string()).into())))));
        str_proto.prop_set("sup", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<sup>{}</sup>", this.to_js_string()).into())))));
        str_proto.prop_set("big", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<big>{}</big>", this.to_js_string()).into())))));
        str_proto.prop_set("small", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<small>{}</small>", this.to_js_string()).into())))));
        str_proto.prop_set("blink", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<blink>{}</blink>", this.to_js_string()).into())))));
        str_proto.prop_set("fixed", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(format!("<tt>{}</tt>", this.to_js_string()).into())))));
        str_proto.prop_set("link", JsValue::HostFunction(Arc::new(|_, this, args| {
            let url = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(format!("<a href=\"{}\">{}</a>", url, this.to_js_string()).into()))
        })));
        str_proto.prop_set("anchor", JsValue::HostFunction(Arc::new(|_, this, args| {
            let name = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(format!("<a name=\"{}\">{}</a>", name, this.to_js_string()).into()))
        })));
        str_proto.prop_set("fontcolor", JsValue::HostFunction(Arc::new(|_, this, args| {
            let color = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(format!("<font color=\"{}\">{}</font>", color, this.to_js_string()).into()))
        })));
        str_proto.prop_set("fontsize", JsValue::HostFunction(Arc::new(|_, this, args| {
            let size = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(format!("<font size=\"{}\">{}</font>", size, this.to_js_string()).into()))
        })));
        map.insert("String".to_string(), str_proto);

        // 3. Array.prototype
        let arr_proto = JsValue::new_object();
        arr_proto.prop_set("push", JsValue::HostFunction(Arc::new(array_push)));
        arr_proto.prop_set("pop", JsValue::HostFunction(Arc::new(array_pop)));
        arr_proto.prop_set("indexOf", JsValue::HostFunction(Arc::new(array_index_of)));
        arr_proto.prop_set("map", JsValue::HostFunction(Arc::new(array_map)));
        arr_proto.prop_set("filter", JsValue::HostFunction(Arc::new(array_filter)));
        arr_proto.prop_set("reduce", JsValue::HostFunction(Arc::new(array_reduce)));
        arr_proto.prop_set("slice", JsValue::HostFunction(Arc::new(array_slice)));
        arr_proto.prop_set("includes", JsValue::HostFunction(Arc::new(array_includes)));
        arr_proto.prop_set("join", JsValue::HostFunction(Arc::new(array_join)));
        arr_proto.prop_set("concat", JsValue::HostFunction(Arc::new(array_concat)));
        arr_proto.prop_set("every", JsValue::HostFunction(Arc::new(array_every)));
        arr_proto.prop_set("some", JsValue::HostFunction(Arc::new(array_some)));
        map.insert("Array".to_string(), arr_proto);

        // 4. Function.prototype
        let func_proto = JsValue::new_object();
        func_proto.prop_set("call", JsValue::HostFunction(Arc::new(function_call)));
        func_proto.prop_set("apply", JsValue::HostFunction(Arc::new(function_apply)));
        func_proto.prop_set("bind", JsValue::HostFunction(Arc::new(function_bind)));
        map.insert("Function".to_string(), func_proto);

        // 5. Symbol constructor & prototype
        let symbol_ctor = JsValue::HostFunction(Arc::new(symbol_constructor));
        map.insert("Symbol".to_string(), symbol_ctor);

        // 6. BigInt constructor & prototype
        let bigint_ctor = JsValue::HostFunction(Arc::new(bigint_constructor));
        map.insert("BigInt".to_string(), bigint_ctor);

        // 7. Promise constructor & prototype
        let promise_ctor = JsValue::HostFunction(Arc::new(promise_constructor));
        map.insert("Promise".to_string(), promise_ctor);

        // 8. Number.prototype
        let num_proto = JsValue::new_object();
        num_proto.prop_set("toString", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(this.to_js_string().into())))));
        num_proto.prop_set("valueOf", JsValue::HostFunction(Arc::new(|_, this, _| Ok(this))));
        map.insert("Number".to_string(), num_proto);

        // 9. Boolean.prototype
        let bool_proto = JsValue::new_object();
        bool_proto.prop_set("toString", JsValue::HostFunction(Arc::new(|_, this, _| Ok(JsValue::String(this.to_js_string().into())))));
        bool_proto.prop_set("valueOf", JsValue::HostFunction(Arc::new(|_, this, _| Ok(this))));
        map.insert("Boolean".to_string(), bool_proto);
    });
}

pub fn get_object_constructor() -> JsValue {
    let ctor = JsValue::new_object();
    GLOBAL_PROTOTYPES.with(|p| {
        if let Some(proto) = p.borrow().get("Object") {
            ctor.prop_set("prototype", proto.clone());
        }
    });
    ctor.prop_set("keys", JsValue::HostFunction(Arc::new(object_keys)));
    ctor.prop_set("assign", JsValue::HostFunction(Arc::new(object_assign)));
    ctor.prop_set("create", JsValue::HostFunction(Arc::new(object_create)));
    ctor.prop_set("getPrototypeOf", JsValue::HostFunction(Arc::new(object_get_prototype_of)));
    ctor.prop_set("defineProperty", JsValue::HostFunction(Arc::new(object_define_property)));
    ctor.prop_set("getOwnPropertyDescriptor", JsValue::HostFunction(Arc::new(object_get_own_property_descriptor)));
    ctor
}

pub fn get_reflect_object() -> JsValue {
    let reflect = JsValue::new_object();
    reflect.prop_set("get", JsValue::HostFunction(Arc::new(|_, _, args| {
        let obj = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
        Ok(obj.prop_get(&key))
    })));
    reflect.prop_set("set", JsValue::HostFunction(Arc::new(|_, _, args| {
        let obj = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
        let val = args.get(2).cloned().unwrap_or(JsValue::Undefined);
        obj.prop_set(&key, val);
        Ok(JsValue::Bool(true))
    })));
    reflect.prop_set("has", JsValue::HostFunction(Arc::new(|_, _, args| {
        let obj = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
        let val = obj.prop_get(&key);
        Ok(JsValue::Bool(!matches!(val, JsValue::Undefined)))
    })));
    reflect.prop_set("deleteProperty", JsValue::HostFunction(Arc::new(|_, _, args| {
        let obj = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
        obj.prop_set(&key, JsValue::Undefined);
        Ok(JsValue::Bool(true))
    })));
    reflect.prop_set("apply", JsValue::HostFunction(Arc::new(function_apply)));
    reflect.prop_set("construct", JsValue::HostFunction(Arc::new(object_create)));
    reflect
}

pub fn get_error_constructor(name: &'static str) -> JsValue {
    let ctor = JsValue::HostFunction(Arc::new(move |_, _, args| {
        let msg = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
        let err_obj = JsValue::new_object();
        err_obj.prop_set("name", JsValue::String(name.into()));
        err_obj.prop_set("message", JsValue::String(msg.into()));
        Ok(err_obj)
    }));
    let proto = JsValue::new_object();
    proto.prop_set("name", JsValue::String(name.into()));
    proto.prop_set("message", JsValue::String("".into()));
    ctor.prop_set("prototype", proto);
    ctor
}

pub fn get_math_object() -> JsValue {
    let math = JsValue::new_object();
    math.prop_set("PI", JsValue::Float(std::f64::consts::PI));
    math.prop_set("E", JsValue::Float(std::f64::consts::E));
    math.prop_set("LN2", JsValue::Float(std::f64::consts::LN_2));
    math.prop_set("LN10", JsValue::Float(std::f64::consts::LN_10));
    math.prop_set("LOG2E", JsValue::Float(std::f64::consts::LOG2_E));
    math.prop_set("LOG10E", JsValue::Float(std::f64::consts::LOG10_E));
    math.prop_set("SQRT1_2", JsValue::Float(std::f64::consts::FRAC_1_SQRT_2));
    math.prop_set("SQRT2", JsValue::Float(std::f64::consts::SQRT_2));

    math.prop_set("abs", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.abs()))
    })));
    math.prop_set("floor", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.floor()))
    })));
    math.prop_set("ceil", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.ceil()))
    })));
    math.prop_set("round", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.round()))
    })));
    math.prop_set("pow", JsValue::HostFunction(Arc::new(|_, _, args| {
        let base = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        let exp = args.get(1).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(base.powf(exp)))
    })));
    math.prop_set("sqrt", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.sqrt()))
    })));
    math.prop_set("max", JsValue::HostFunction(Arc::new(|_, _, args| {
        if args.is_empty() { return Ok(JsValue::Float(f64::NEG_INFINITY)); }
        let mut max_val = f64::NEG_INFINITY;
        for a in args {
            let n = a.to_number();
            if n.is_nan() { return Ok(JsValue::Float(f64::NAN)); }
            if n > max_val { max_val = n; }
        }
        Ok(JsValue::from_f64(max_val))
    })));
    math.prop_set("min", JsValue::HostFunction(Arc::new(|_, _, args| {
        if args.is_empty() { return Ok(JsValue::Float(f64::INFINITY)); }
        let mut min_val = f64::INFINITY;
        for a in args {
            let n = a.to_number();
            if n.is_nan() { return Ok(JsValue::Float(f64::NAN)); }
            if n < min_val { min_val = n; }
        }
        Ok(JsValue::from_f64(min_val))
    })));
    math.prop_set("trunc", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(n.trunc()))
    })));

    math
}

pub fn get_number_constructor() -> JsValue {
    let num = JsValue::new_object();
    num.prop_set("isNaN", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::Bool(n.is_nan()))
    })));
    num.prop_set("isFinite", JsValue::HostFunction(Arc::new(|_, _, args| {
        let n = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
        Ok(JsValue::Bool(n.is_finite()))
    })));
    num.prop_set("parseInt", JsValue::HostFunction(Arc::new(|_, _, args| {
        let s = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
        let rad = args.get(1).map(|v| v.to_number() as u32).unwrap_or(10);
        let parsed = i32::from_str_radix(s.trim(), rad).unwrap_or(0);
        Ok(JsValue::Int(parsed))
    })));
    num.prop_set("parseFloat", JsValue::HostFunction(Arc::new(|_, _, args| {
        let s = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
        let parsed = s.trim().parse::<f64>().unwrap_or(f64::NAN);
        Ok(JsValue::from_f64(parsed))
    })));
    num
}

pub fn get_map_constructor() -> JsValue {
    let map_ctor = JsValue::new_object();
    let map_proto = JsValue::new_object();
    
    map_proto.prop_set("set", JsValue::HostFunction(Arc::new(|_, this, args| {
        let key = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let val = args.get(1).cloned().unwrap_or(JsValue::Undefined);
        this.prop_set(&key.to_js_string(), val);
        Ok(this)
    })));
    map_proto.prop_set("get", JsValue::HostFunction(Arc::new(|_, this, args| {
        let key = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        Ok(this.prop_get(&key.to_js_string()))
    })));
    map_proto.prop_set("has", JsValue::HostFunction(Arc::new(|_, this, args| {
        let key = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let v = this.prop_get(&key.to_js_string());
        Ok(JsValue::Bool(!matches!(v, JsValue::Undefined)))
    })));
    map_proto.prop_set("delete", JsValue::HostFunction(Arc::new(|_, this, args| {
        let key = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        this.prop_set(&key.to_js_string(), JsValue::Undefined);
        Ok(JsValue::Bool(true))
    })));

    map_ctor.prop_set("prototype", map_proto);
    map_ctor
}

pub fn get_set_constructor() -> JsValue {
    let set_ctor = JsValue::new_object();
    let set_proto = JsValue::new_object();

    set_proto.prop_set("add", JsValue::HostFunction(Arc::new(|_, this, args| {
        let val = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        this.prop_set(&val.to_js_string(), JsValue::Bool(true));
        Ok(this)
    })));
    set_proto.prop_set("has", JsValue::HostFunction(Arc::new(|_, this, args| {
        let val = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let v = this.prop_get(&val.to_js_string());
        Ok(JsValue::Bool(matches!(v, JsValue::Bool(true))))
    })));
    set_proto.prop_set("delete", JsValue::HostFunction(Arc::new(|_, this, args| {
        let val = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        this.prop_set(&val.to_js_string(), JsValue::Undefined);
        Ok(JsValue::Bool(true))
    })));

    set_ctor.prop_set("prototype", set_proto);
    set_ctor
}

fn object_to_string(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, _args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    Ok(JsValue::String("[object Object]".into()))
}

fn string_index_of(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let this_str = this.to_js_string();
    let search = args.get(0).cloned().unwrap_or(JsValue::Undefined).to_js_string();
    let start_idx = args.get(1).map(|v| v.to_number() as usize).unwrap_or(0);
    
    if start_idx > this_str.len() {
        return Ok(JsValue::Int(-1));
    }
    if let Some(pos) = this_str[start_idx..].find(&search) {
        Ok(JsValue::Int((start_idx + pos) as i32))
    } else {
        Ok(JsValue::Int(-1))
    }
}

fn string_slice(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let this_str = this.to_js_string();
    let chars: Vec<char> = this_str.chars().collect();
    let len = chars.len() as i32;
    let start = args.get(0).map(|v| v.to_number() as i32).unwrap_or(0);
    let end = args.get(1).map(|v| v.to_number() as i32).unwrap_or(len);
    
    let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
    let end_idx = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
    
    if start_idx >= end_idx {
        Ok(JsValue::String("".into()))
    } else {
        let sliced: String = chars[start_idx..end_idx].iter().collect();
        Ok(JsValue::String(sliced.into()))
    }
}

fn string_char_at(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let this_str = this.to_js_string();
    let pos = args.get(0).map(|v| v.to_number() as i32).unwrap_or(0);
    let chars: Vec<char> = this_str.chars().collect();
    if pos < 0 || pos >= chars.len() as i32 {
        Ok(JsValue::String("".into()))
    } else {
        Ok(JsValue::String(chars[pos as usize].to_string().into()))
    }
}

fn string_replace(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let from = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    let to = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
    Ok(JsValue::String(s.replacen(&from, &to, 1).into()))
}

fn string_replace_all(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let from = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    let to = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
    Ok(JsValue::String(s.replace(&from, &to).into()))
}

fn string_split(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let sep = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    let parts: Vec<JsValue> = s.split(&sep).map(|p| JsValue::String(p.into())).collect();
    Ok(JsValue::Array(Arc::new(RwLock::new(parts))))
}

fn string_trim(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, _args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    Ok(JsValue::String(s.trim().into()))
}

fn string_starts_with(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let pat = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    Ok(JsValue::Bool(s.starts_with(&pat)))
}

fn string_ends_with(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let pat = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    Ok(JsValue::Bool(s.ends_with(&pat)))
}

fn string_includes(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let pat = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
    Ok(JsValue::Bool(s.contains(&pat)))
}

fn string_pad_start(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let char_count = s.chars().count();
    let target_len = args.get(0).map(|v| v.to_number() as usize).unwrap_or(char_count);
    let pad_str = args.get(1).map(|v| v.to_js_string()).unwrap_or(" ".to_string());
    if char_count >= target_len || pad_str.is_empty() { return Ok(JsValue::String(s.into())); }
    let needed = target_len - char_count;
    let pad_chars: Vec<char> = pad_str.chars().collect();
    let mut pad = String::new();
    for i in 0..needed {
        pad.push(pad_chars[i % pad_chars.len()]);
    }
    Ok(JsValue::String(format!("{}{}", pad, s).into()))
}

fn string_pad_end(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let s = this.to_js_string();
    let char_count = s.chars().count();
    let target_len = args.get(0).map(|v| v.to_number() as usize).unwrap_or(char_count);
    let pad_str = args.get(1).map(|v| v.to_js_string()).unwrap_or(" ".to_string());
    if char_count >= target_len || pad_str.is_empty() { return Ok(JsValue::String(s.into())); }
    let needed = target_len - char_count;
    let pad_chars: Vec<char> = pad_str.chars().collect();
    let mut pad = String::new();
    for i in 0..needed {
        pad.push(pad_chars[i % pad_chars.len()]);
    }
    Ok(JsValue::String(format!("{}{}", s, pad).into()))
}

fn array_push(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    if let JsValue::Array(arr) = this {
        let mut g = arr.write().unwrap();
        for arg in args {
            g.push(arg);
        }
        Ok(JsValue::Int(g.len() as i32))
    } else {
        Ok(JsValue::Int(0))
    }
}

fn array_pop(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, _args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    if let JsValue::Array(arr) = this {
        let mut g = arr.write().unwrap();
        let popped = g.pop().unwrap_or(JsValue::Undefined);
        Ok(popped)
    } else {
        Ok(JsValue::Undefined)
    }
}

fn array_index_of(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    if let JsValue::Array(arr) = this {
        let search = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let start = args.get(1).map(|v| v.to_number() as i32).unwrap_or(0);
        let g = arr.read().unwrap();
        let len = g.len() as i32;
        let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        
        for idx in start_idx..g.len() {
            if JsValue::js_strict_eq(&g[idx], &search) {
                return Ok(JsValue::Int(idx as i32));
            }
        }
        Ok(JsValue::Int(-1))
    } else {
        Ok(JsValue::Int(-1))
    }
}

fn array_map(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let cb = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Array(arr) = this {
        let items = arr.read().unwrap().clone();
        let mut out = Vec::with_capacity(items.len());
        for (i, item) in items.into_iter().enumerate() {
            let res = function_call(caller, cb.clone(), vec![JsValue::Undefined, item, JsValue::Int(i as i32)])?;
            out.push(res);
        }
        Ok(JsValue::Array(Arc::new(RwLock::new(out))))
    } else {
        Ok(JsValue::Array(Arc::new(RwLock::new(vec![]))))
    }
}

fn array_filter(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let cb = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Array(arr) = this {
        let items = arr.read().unwrap().clone();
        let mut out = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let res = function_call(caller, cb.clone(), vec![JsValue::Undefined, item.clone(), JsValue::Int(i as i32)])?;
            if res.is_truthy() {
                out.push(item);
            }
        }
        Ok(JsValue::Array(Arc::new(RwLock::new(out))))
    } else {
        Ok(JsValue::Array(Arc::new(RwLock::new(vec![]))))
    }
}

fn array_reduce(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let cb = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let mut acc = args.get(1).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Array(arr) = this {
        let items = arr.read().unwrap().clone();
        for (i, item) in items.into_iter().enumerate() {
            acc = function_call(caller, cb.clone(), vec![JsValue::Undefined, acc, item, JsValue::Int(i as i32)])?;
        }
    }
    Ok(acc)
}

fn array_slice(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    if let JsValue::Array(arr) = this {
        let g = arr.read().unwrap();
        let len = g.len() as i32;
        let start = args.get(0).map(|v| v.to_number() as i32).unwrap_or(0);
        let end = args.get(1).map(|v| v.to_number() as i32).unwrap_or(len);
        let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let end_idx = if end < 0 { (len + end).max(0) } else { end.min(len) } as usize;
        let sliced = if start_idx < end_idx { g[start_idx..end_idx].to_vec() } else { vec![] };
        Ok(JsValue::Array(Arc::new(RwLock::new(sliced))))
    } else {
        Ok(JsValue::Array(Arc::new(RwLock::new(vec![]))))
    }
}

fn array_includes(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    if let JsValue::Array(arr) = this {
        let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let g = arr.read().unwrap();
        for item in g.iter() {
            if JsValue::js_strict_eq(item, &target) { return Ok(JsValue::Bool(true)); }
        }
    }
    Ok(JsValue::Bool(false))
}

fn array_join(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let sep = args.get(0).map(|v| v.to_js_string()).unwrap_or(",".to_string());
    if let JsValue::Array(arr) = this {
        let g = arr.read().unwrap();
        let s = g.iter().map(|v| v.to_js_string()).collect::<Vec<_>>().join(&sep);
        Ok(JsValue::String(s.into()))
    } else {
        Ok(JsValue::String("".into()))
    }
}

fn array_concat(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let mut out = Vec::new();
    if let JsValue::Array(arr) = this {
        out.extend(arr.read().unwrap().clone());
    }
    for arg in args {
        if let JsValue::Array(a) = arg {
            out.extend(a.read().unwrap().clone());
        } else {
            out.push(arg);
        }
    }
    Ok(JsValue::Array(Arc::new(RwLock::new(out))))
}

fn array_every(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let cb = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Array(arr) = this {
        let items = arr.read().unwrap().clone();
        for (i, item) in items.into_iter().enumerate() {
            let res = function_call(caller, cb.clone(), vec![JsValue::Undefined, item, JsValue::Int(i as i32)])?;
            if !res.is_truthy() { return Ok(JsValue::Bool(false)); }
        }
    }
    Ok(JsValue::Bool(true))
}

fn array_some(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let cb = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Array(arr) = this {
        let items = arr.read().unwrap().clone();
        for (i, item) in items.into_iter().enumerate() {
            let res = function_call(caller, cb.clone(), vec![JsValue::Undefined, item, JsValue::Int(i as i32)])?;
            if res.is_truthy() { return Ok(JsValue::Bool(true)); }
        }
    }
    Ok(JsValue::Bool(false))
}

fn function_call(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let raw_this = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let rest_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };

    let is_strict = caller.data().call_stack.last().map(|f| f.is_strict).unwrap_or(false);
    let this_arg = if !is_strict && (matches!(raw_this, JsValue::Undefined) || matches!(raw_this, JsValue::Null)) {
        caller.data().globals.get("globalThis").cloned().unwrap_or(raw_this)
    } else {
        raw_this
    };

    match this {
        JsValue::Function(idx) => {
            if let Some(wasmtime::Extern::Func(func)) = caller.get_export(&format!("func_{}", idx)) {
                caller.data_mut().call_stack.push(CallFrame {
                    this: this_arg,
                    args: rest_args,
                    is_strict,
                });
                
                let mut results = vec![wasmtime::Val::null_extern_ref()];
                func.call(&mut *caller, &[], &mut results)?;
                
                caller.data_mut().call_stack.pop();
                
                let ret = match &results[0] {
                    wasmtime::Val::ExternRef(Some(r)) => {
                        let data = r.data(&*caller)?;
                        match data {
                            Some(d) => d.downcast_ref::<JsValue>().cloned().unwrap_or(JsValue::Undefined),
                            None    => JsValue::Null,
                        }
                    }
                    _ => JsValue::Undefined,
                };
                Ok(ret)
            } else {
                anyhow::bail!("Func export func_{} not found", idx);
            }
        }
        JsValue::HostFunction(f) => {
            f(caller, this_arg, rest_args)
        }
        _ => {
            Ok(JsValue::Undefined)
        }
    }
}

fn object_keys(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Object(obj) = target {
        let guard = obj.read().unwrap();
        let keys: Vec<JsValue> = guard.props.keys().map(|k| JsValue::String(k.clone().into())).collect();
        Ok(JsValue::Array(Arc::new(RwLock::new(keys))))
    } else {
        Ok(JsValue::Array(Arc::new(RwLock::new(vec![]))))
    }
}

fn object_assign(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Object(target_obj) = &target {
        for src in &args[1..] {
            if let JsValue::Object(src_obj) = src {
                let src_guard = src_obj.read().unwrap();
                let mut target_guard = target_obj.write().unwrap();
                for (k, v) in src_guard.props.iter() {
                    target_guard.props.insert(k.clone(), v.clone());
                }
            }
        }
    }
    Ok(target)
}

fn object_create(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let proto = args.get(0).cloned().unwrap_or(JsValue::Null);
    let new_obj = JsValue::new_object();
    if let JsValue::Object(new_inner) = &new_obj {
        if let JsValue::Object(proto_inner) = proto {
            new_inner.write().unwrap().proto = Some(proto_inner);
        }
    }
    Ok(new_obj)
}

fn object_get_prototype_of(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    if let JsValue::Object(obj) = target {
        let guard = obj.read().unwrap();
        if let Some(proto) = &guard.proto {
            Ok(JsValue::Object(proto.clone()))
        } else {
            Ok(JsValue::Null)
        }
    } else {
        Ok(JsValue::Null)
    }
}

fn object_define_property(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
    let desc_val = args.get(2).cloned().unwrap_or(JsValue::Undefined);

    if let JsValue::Object(obj) = &target {
        let val = desc_val.prop_get("value");
        let writable = desc_val.prop_get("writable").is_truthy();
        let enumerable = desc_val.prop_get("enumerable").is_truthy();
        let configurable = desc_val.prop_get("configurable").is_truthy();
        let get = desc_val.prop_get("get");
        let set = desc_val.prop_get("set");

        let desc = PropertyDescriptor {
            value: if matches!(val, JsValue::Undefined) { None } else { Some(val.clone()) },
            writable,
            enumerable,
            configurable,
            get: if matches!(get, JsValue::Undefined) { None } else { Some(get) },
            set: if matches!(set, JsValue::Undefined) { None } else { Some(set) },
        };

        let mut guard = obj.write().unwrap();
        guard.descriptors.insert(key.clone(), desc);
        if !matches!(val, JsValue::Undefined) {
            guard.props.insert(key, val);
        }
    }
    Ok(target)
}

fn object_get_own_property_descriptor(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let target = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let key = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
    if let JsValue::Object(obj) = target {
        let guard = obj.read().unwrap();
        if let Some(desc) = guard.descriptors.get(&key) {
            let res = JsValue::new_object();
            res.prop_set("writable", JsValue::Bool(desc.writable));
            res.prop_set("enumerable", JsValue::Bool(desc.enumerable));
            res.prop_set("configurable", JsValue::Bool(desc.configurable));
            if let Some(v) = &desc.value { res.prop_set("value", v.clone()); }
            return Ok(res);
        } else if let Some(v) = guard.props.get(&key) {
            let res = JsValue::new_object();
            res.prop_set("writable", JsValue::Bool(true));
            res.prop_set("enumerable", JsValue::Bool(true));
            res.prop_set("configurable", JsValue::Bool(true));
            res.prop_set("value", v.clone());
            return Ok(res);
        }
    }
    Ok(JsValue::Undefined)
}

fn function_apply(caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let this_arg = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let arg_array = args.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut call_args = Vec::new();
    if let JsValue::Array(arr) = arg_array {
        call_args = arr.read().unwrap().clone();
    }
    let mut combined_args = vec![this_arg];
    combined_args.extend(call_args);
    function_call(caller, this, combined_args)
}

fn function_bind(_caller: &mut wasmtime::Caller<'_, HostState>, this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let bound_this = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let bound_args = if args.len() > 1 { args[1..].to_vec() } else { vec![] };
    let target_fn = this;

    let host_fn: HostFn = Arc::new(move |caller: &mut wasmtime::Caller<'_, HostState>, _this_arg: JsValue, extra_args: Vec<JsValue>| {
        let mut all_args = vec![bound_this.clone()];
        all_args.extend(bound_args.clone());
        all_args.extend(extra_args);
        function_call(caller, target_fn.clone(), all_args)
    });

    Ok(JsValue::HostFunction(host_fn))
}

static SYMBOL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn symbol_constructor(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, _args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let id = SYMBOL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(JsValue::Symbol(id))
}

fn bigint_constructor(_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let val = args.get(0).cloned().unwrap_or(JsValue::Int(0));
    let num = val.to_number() as i128;
    Ok(JsValue::BigInt(num))
}

fn promise_constructor(caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, args: Vec<JsValue>) -> anyhow::Result<JsValue> {
    let executor = args.get(0).cloned().unwrap_or(JsValue::Undefined);
    let promise_obj = JsValue::new_object();
    promise_obj.prop_set("status", JsValue::String("fulfilled".into()));
    promise_obj.prop_set("value", JsValue::Undefined);

    let res_obj = promise_obj.clone();
    let resolve_fn: HostFn = Arc::new(move |_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, res_args: Vec<JsValue>| {
        let val = res_args.get(0).cloned().unwrap_or(JsValue::Undefined);
        res_obj.prop_set("status", JsValue::String("fulfilled".into()));
        res_obj.prop_set("value", val);
        Ok(JsValue::Undefined)
    });

    let rej_obj = promise_obj.clone();
    let reject_fn: HostFn = Arc::new(move |_caller: &mut wasmtime::Caller<'_, HostState>, _this: JsValue, rej_args: Vec<JsValue>| {
        let val = rej_args.get(0).cloned().unwrap_or(JsValue::Undefined);
        rej_obj.prop_set("status", JsValue::String("rejected".into()));
        rej_obj.prop_set("value", val);
        Ok(JsValue::Undefined)
    });

    if !matches!(executor, JsValue::Undefined) {
        function_call(caller, executor, vec![JsValue::Undefined, JsValue::HostFunction(resolve_fn), JsValue::HostFunction(reject_fn)])?;
    }

    Ok(promise_obj)
}

pub fn get_intl_object() -> JsValue {
    let intl = JsValue::new_object();

    // 1. Intl.defaultLocale() -> "en-US"
    intl.prop_set("defaultLocale", JsValue::HostFunction(Arc::new(|_, _, _| {
        Ok(JsValue::String("en-US".into()))
    })));

    // 2. Intl.getCanonicalLocales(locales)
    intl.prop_set("getCanonicalLocales", JsValue::HostFunction(Arc::new(|_, _, args| {
        let loc_arg = args.get(0).cloned().unwrap_or(JsValue::Undefined);
        let mut result_arr = Vec::new();
        let list = match loc_arg {
            JsValue::String(s) => vec![s.to_string()],
            JsValue::Array(arr) => arr.read().unwrap().iter().map(|v| v.to_js_string()).collect(),
            _ => vec!["en-US".to_string()],
        };
        for l in list {
            if let Ok(loc) = l.parse::<icu::locid::Locale>() {
                result_arr.push(JsValue::String(loc.to_string().into()));
            } else {
                result_arr.push(JsValue::String(l.into()));
            }
        }
        let arr_obj = JsValue::Array(Arc::new(RwLock::new(result_arr)));
        Ok(arr_obj)
    })));

    // 3. Intl.Locale constructor
    let locale_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("baseName", JsValue::String(loc.to_string().into()));
        obj.prop_set("language", JsValue::String(loc.id.language.to_string().into()));
        let script_str = loc.id.script.map(|s| s.to_string()).unwrap_or_default();
        obj.prop_set("script", if script_str.is_empty() { JsValue::Undefined } else { JsValue::String(script_str.into()) });
        let region_str = loc.id.region.map(|r| r.to_string()).unwrap_or_default();
        obj.prop_set("region", if region_str.is_empty() { JsValue::Undefined } else { JsValue::String(region_str.into()) });
        obj.prop_set("toString", JsValue::HostFunction(Arc::new(move |_, this, _| {
            Ok(this.prop_get("baseName"))
        })));
        Ok(obj)
    }));
    intl.prop_set("Locale", locale_ctor);

    // 4. Intl.NumberFormat constructor
    let number_format_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("format", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let val = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
            if val.is_nan() {
                return Ok(JsValue::String("NaN".into()));
            }
            if val.is_infinite() {
                return Ok(JsValue::String(if val < 0.0 { "-∞" } else { "∞" }.into()));
            }
            Ok(JsValue::String(val.to_string().into()))
        })));
        obj.prop_set("resolvedOptions", JsValue::HostFunction(Arc::new(move |_, this, _| {
            let opts = JsValue::new_object();
            opts.prop_set("locale", this.prop_get("locale"));
            opts.prop_set("style", JsValue::String("decimal".into()));
            Ok(opts)
        })));
        Ok(obj)
    }));
    intl.prop_set("NumberFormat", number_format_ctor);

    // 5. Intl.DateTimeFormat constructor
    let datetime_format_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("format", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let val = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(val.into()))
        })));
        obj.prop_set("resolvedOptions", JsValue::HostFunction(Arc::new(move |_, this, _| {
            let opts = JsValue::new_object();
            opts.prop_set("locale", this.prop_get("locale"));
            opts.prop_set("calendar", JsValue::String("gregory".into()));
            opts.prop_set("timeZone", JsValue::String("UTC".into()));
            Ok(opts)
        })));
        Ok(obj)
    }));
    intl.prop_set("DateTimeFormat", datetime_format_ctor);

    // 6. Intl.Collator constructor
    let collator_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("compare", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let a = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            let b = args.get(1).map(|v| v.to_js_string()).unwrap_or_default();
            let cmp = match icu::collator::Collator::try_new(&loc.clone().into(), Default::default()) {
                Ok(c) => c.compare(&a, &b) as i32,
                Err(_) => a.cmp(&b) as i32,
            };
            Ok(JsValue::from_f64(cmp as f64))
        })));
        obj.prop_set("resolvedOptions", JsValue::HostFunction(Arc::new(move |_, this, _| {
            let opts = JsValue::new_object();
            opts.prop_set("locale", this.prop_get("locale"));
            opts.prop_set("usage", JsValue::String("sort".into()));
            opts.prop_set("sensitivity", JsValue::String("variant".into()));
            Ok(opts)
        })));
        Ok(obj)
    }));
    intl.prop_set("Collator", collator_ctor);

    // 7. Intl.PluralRules constructor
    let plural_rules_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("select", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let val = args.get(0).map(|v| v.to_number() as i64).unwrap_or(0);
            let cat = match icu::plurals::PluralRules::try_new_cardinal(&loc.clone().into()) {
                Ok(pr) => match pr.category_for(val) {
                    icu::plurals::PluralCategory::Zero => "zero",
                    icu::plurals::PluralCategory::One => "one",
                    icu::plurals::PluralCategory::Two => "two",
                    icu::plurals::PluralCategory::Few => "few",
                    icu::plurals::PluralCategory::Many => "many",
                    icu::plurals::PluralCategory::Other => "other",
                },
                Err(_) => if val == 1 { "one" } else { "other" },
            };
            Ok(JsValue::String(cat.into()))
        })));
        obj.prop_set("resolvedOptions", JsValue::HostFunction(Arc::new(move |_, this, _| {
            let opts = JsValue::new_object();
            opts.prop_set("locale", this.prop_get("locale"));
            opts.prop_set("pluralCategories", JsValue::Array(Arc::new(RwLock::new(vec![
                JsValue::String("one".into()),
                JsValue::String("other".into()),
            ]))));
            Ok(opts)
        })));
        Ok(obj)
    }));
    intl.prop_set("PluralRules", plural_rules_ctor);

    // 8. Intl.Segmenter constructor
    let segmenter_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("segment", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let input = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            let seg_result = JsValue::new_object();
            let mut segs = Vec::new();
            let segmenter = icu::segmenter::GraphemeClusterSegmenter::new();
            let mut prev = 0;
            for pos in segmenter.segment_str(&input) {
                if pos > prev {
                    let chunk = &input[prev..pos];
                    let seg_item = JsValue::new_object();
                    seg_item.prop_set("segment", JsValue::String(chunk.into()));
                    seg_item.prop_set("index", JsValue::from_f64(prev as f64));
                    segs.push(seg_item);
                    prev = pos;
                }
            }
            seg_result.prop_set("segments", JsValue::Array(Arc::new(RwLock::new(segs))));
            Ok(seg_result)
        })));
        Ok(obj)
    }));
    intl.prop_set("Segmenter", segmenter_ctor);

    // 9. Intl.ListFormat constructor
    let list_format_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("format", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let list_arg = args.get(0).cloned().unwrap_or(JsValue::Undefined);
            let items: Vec<String> = match list_arg {
                JsValue::Array(arr) => arr.read().unwrap().iter().map(|v| v.to_js_string()).collect(),
                _ => Vec::new(),
            };
            let formatted = items.join(", ");
            Ok(JsValue::String(formatted.into()))
        })));
        Ok(obj)
    }));
    intl.prop_set("ListFormat", list_format_ctor);

    // 10. Intl.RelativeTimeFormat constructor
    let relative_time_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("format", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let val = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
            let unit = args.get(1).map(|v| v.to_js_string()).unwrap_or_else(|| "day".to_string());
            let formatted = format!("{} {}", val, unit);
            Ok(JsValue::String(formatted.into()))
        })));
        Ok(obj)
    }));
    intl.prop_set("RelativeTimeFormat", relative_time_ctor);

    // 11. Intl.DisplayNames constructor
    let display_names_ctor = JsValue::HostFunction(Arc::new(|_, _, args| {
        let tag = args.get(0).map(|v| v.to_js_string()).unwrap_or_else(|| "en-US".to_string());
        let loc = tag.parse::<icu::locid::Locale>().unwrap_or_else(|_| "en-US".parse().unwrap());
        let obj = JsValue::new_object();
        obj.prop_set("locale", JsValue::String(loc.to_string().into()));
        obj.prop_set("of", JsValue::HostFunction(Arc::new(move |_, _, args| {
            let code = args.get(0).map(|v| v.to_js_string()).unwrap_or_default();
            Ok(JsValue::String(code.into()))
        })));
        Ok(obj)
    }));
    intl.prop_set("DisplayNames", display_names_ctor);

    intl
}

fn alloc_wasm_memory(caller: &mut wasmtime::Caller<'_, HostState>, size: u32) -> anyhow::Result<i32> {
    let (ptr, mem_ptr_after, memory_opt) = {
        let state = caller.data_mut();
        state.mem_ptr = (state.mem_ptr + 7) & !7;
        let ptr = state.mem_ptr as i32;
        let mem_ptr_after = state.mem_ptr + size;
        let memory_opt = state.memory;
        (ptr, mem_ptr_after, memory_opt)
    };

    if let Some(memory) = memory_opt {
        let current_bytes = memory.data_size(&*caller) as u32;
        if mem_ptr_after > current_bytes {
            let diff = mem_ptr_after - current_bytes;
            let pages_needed = (diff + 65535) / 65536;
            memory.grow(&mut *caller, pages_needed as u64)?;
            
            let data_ptr = memory.data_mut(&mut *caller).as_mut_ptr();
            let data_len = memory.data_size(&*caller);
            crate::js_value::ACTIVE_MEMORY.with(|cell| {
                cell.set(Some((data_ptr, data_len)));
            });
        }
    }

    let state = caller.data_mut();
    state.mem_ptr = mem_ptr_after;

    Ok(ptr)
}

pub fn get_array_buffer_constructor() -> JsValue {
    JsValue::HostFunction(Arc::new(|caller, _, args| {
        let length_val = args.get(0).cloned().unwrap_or(JsValue::Int(0));
        let length = match length_val {
            JsValue::Int(i) => i.max(0) as u32,
            JsValue::Float(f) => f.max(0.0) as u32,
            _ => 0,
        };
        let ptr = alloc_wasm_memory(caller, length)?;
        let obj = JsValue::new_object();
        obj.prop_set("__ptr__", JsValue::Int(ptr));
        obj.prop_set("byteLength", JsValue::Int(length as i32));
        Ok(obj)
    }))
}

pub fn get_typed_array_constructor(name: &'static str, element_size: u32, is_float: bool) -> JsValue {
    JsValue::HostFunction(Arc::new(move |caller, _, args| {
        let first_arg = args.get(0).cloned().unwrap_or(JsValue::Int(0));
        let obj = JsValue::new_object();
        
        let is_buffer = if let JsValue::Object(o) = &first_arg {
            o.read().unwrap().props.contains_key("__ptr__")
        } else {
            false
        };
        
        if is_buffer {
            let buffer_obj = first_arg;
            let ptr = if let JsValue::Object(o) = &buffer_obj {
                match o.read().unwrap().props.get("__ptr__") {
                    Some(JsValue::Int(p)) => *p,
                    _ => 0,
                }
            } else {
                0
            };
            let byte_offset_val = args.get(1).cloned().unwrap_or(JsValue::Int(0));
            let byte_offset = match byte_offset_val {
                JsValue::Int(i) => i.max(0) as u32,
                JsValue::Float(f) => f.max(0.0) as u32,
                _ => 0,
            };
            let length_val = args.get(2).cloned().unwrap_or(JsValue::Undefined);
            let length = match length_val {
                JsValue::Int(i) => Some(i.max(0) as u32),
                JsValue::Float(f) => Some(f.max(0.0) as u32),
                _ => None,
            };
            
            let buffer_byte_length = if let JsValue::Object(o) = &buffer_obj {
                match o.read().unwrap().props.get("byteLength") {
                    Some(JsValue::Int(l)) => *l as u32,
                    _ => 0,
                }
            } else {
                0
            };
            
            let actual_length = length.unwrap_or_else(|| {
                (buffer_byte_length.saturating_sub(byte_offset)) / element_size
            });
            
            obj.prop_set("__ptr__", JsValue::Int(ptr + byte_offset as i32));
            obj.prop_set("__element_size__", JsValue::Int(element_size as i32));
            obj.prop_set("__is_float__", JsValue::Bool(is_float));
            obj.prop_set("length", JsValue::Int(actual_length as i32));
            obj.prop_set("byteLength", JsValue::Int((actual_length * element_size) as i32));
            obj.prop_set("byteOffset", JsValue::Int(byte_offset as i32));
            obj.prop_set("buffer", buffer_obj);
        } else {
            let length = match first_arg {
                JsValue::Int(i) => i.max(0) as u32,
                JsValue::Float(f) => f.max(0.0) as u32,
                _ => 0,
            };
            let byte_length = length * element_size;
            let ptr = alloc_wasm_memory(caller, byte_length)?;
            
            let buffer_obj = JsValue::new_object();
            buffer_obj.prop_set("__ptr__", JsValue::Int(ptr));
            buffer_obj.prop_set("byteLength", JsValue::Int(byte_length as i32));
            
            obj.prop_set("__ptr__", JsValue::Int(ptr));
            obj.prop_set("__element_size__", JsValue::Int(element_size as i32));
            obj.prop_set("__is_float__", JsValue::Bool(is_float));
            obj.prop_set("length", JsValue::Int(length as i32));
            obj.prop_set("byteLength", JsValue::Int(byte_length as i32));
            obj.prop_set("byteOffset", JsValue::Int(0));
            obj.prop_set("buffer", buffer_obj);
        }
        
        GLOBAL_PROTOTYPES.with(|p| {
            if let Some(proto) = p.borrow().get(name) {
                obj.prop_set("__proto__", proto.clone());
            }
        });
        
        Ok(obj)
    }))
}
