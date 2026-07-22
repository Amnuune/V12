//! v12-emitter: translates V12 SSA IR into WasmGC bytecode using wasm-encoder.
//!
//! The WasmGC type system used:
//!
//!   $JsValue = (struct (field $tag i32) (field $payload (ref any)))
//!
//! Tag constants (matching the runtime):
//!   0 = Undefined
//!   1 = Null
//!   2 = Bool    (payload = i31ref with value 0 or 1)
//!   3 = Int     (payload = i31ref with integer value)
//!   4 = Float   (payload = (ref $BoxedFloat))
//!   5 = String  (payload = (ref $JsString))
//!   6 = Object  (payload = (ref $JsObject))
//!   7 = Array   (payload = (ref $JsArray))
//!   8 = Func    (payload = (ref func))

pub mod emitter;
pub mod types;

pub use emitter::Emitter;
