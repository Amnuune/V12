//! Type index constants for WasmGC types emitted by V12.

/// WasmGC type indices (matched in emitter.rs and runtime.rs).
pub const TYPE_JS_VALUE:    u32 = 0;
pub const TYPE_BOXED_FLOAT: u32 = 1;
pub const TYPE_JS_STRING:   u32 = 2;
pub const TYPE_JS_OBJECT:   u32 = 3;
pub const TYPE_JS_ARRAY:    u32 = 4;
pub const TYPE_JS_FUNC:     u32 = 5;

/// $JsValue tag constants.
pub const TAG_UNDEFINED: i32 = 0;
pub const TAG_NULL:      i32 = 1;
pub const TAG_BOOL:      i32 = 2;
pub const TAG_INT:       i32 = 3;
pub const TAG_FLOAT:     i32 = 4;
pub const TAG_STRING:    i32 = 5;
pub const TAG_OBJECT:    i32 = 6;
pub const TAG_ARRAY:     i32 = 7;
pub const TAG_FUNC:      i32 = 8;
