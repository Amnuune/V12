//! v12-ir: SSA Intermediate Representation for the V12 engine.
//!
//! Structure:
//!   Module → [Function] → [BasicBlock] → [Instruction]
//!
//! Values are identified by `ValueId` (SSA: each value defined exactly once).
//! Blocks are identified by `BlockId`.

pub mod builder;
pub mod types;

pub use builder::IrBuilder;
pub use types::*;

use id_arena::{Arena, Id};

pub type ValueId = Id<Value>;
pub type BlockId = Id<BasicBlock>;
pub type FuncId  = Id<Function>;

// ─── Value ───────────────────────────────────────────────────────────────────

/// A single SSA value definition.
#[derive(Debug, Clone)]
pub struct Value {
    pub ty:   JsType,
    pub kind: ValueKind,
}

/// What produced this value.
#[derive(Debug, Clone)]
pub enum ValueKind {
    /// Literal constant.
    Const(Const),
    /// Result of an instruction in a specific block.
    Inst { block: BlockId, index: usize },
    /// Function parameter (index into param list).
    Param(usize),
    /// Φ-node at a block entry.
    Phi(BlockId),
    /// Undefined placeholder (used for declared-but-not-yet-assigned vars).
    Undef,
}

// ─── Constants ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    /// Interned string index into `Module::strings`.
    String(u32),
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// Coarse type info used to guide code generation.
/// We use `Any` for untyped/dynamic JS values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsType {
    /// Statically known to be an i32 (after unboxing).
    Int,
    /// Statically known to be an f64.
    Float,
    /// Statically known to be a boolean.
    Bool,
    /// Unknown / mixed — must use the $JsValue tagged-union at runtime.
    Any,
    /// Object compiled with a statically known WasmGC shape ID.
    ObjectShape(u32),
    /// Vector128 type for SIMD loop vectorization.
    Vector128,
    /// TypedArray hint
    TypedArray { element_size: u32, is_float: bool },
}

// ─── Instructions ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Instruction {
    pub result: Option<ValueId>,
    pub op:     Op,
}

#[derive(Debug, Clone)]
pub enum Op {
    // ── Constants ──
    /// Load a constant into a value.
    Const(Const),

    // ── Arithmetic / Logic ──
    /// Binary operation between two JS values.
    BinOp { op: BinOp, lhs: ValueId, rhs: ValueId },
    /// Unary operation.
    UnOp  { op: UnOp,  val: ValueId },

    // ── Variables ──
    /// Declare a local variable slot; returns its slot index.
    LocalNew { name: String },
    /// Read a local variable slot.
    LocalGet(u32),
    /// Write a local variable slot.
    LocalSet(u32, ValueId),

    // ── Objects ──
    /// Allocate a new empty JS object.
    ObjectNew,
    /// Allocate a new JS object with a statically known shape.
    ObjectShapeNew { shape_id: u32, args: Vec<ValueId> },
    /// Get a property by name string.
    PropGet { obj: ValueId, key: ValueId },
    /// Set a property by name string.
    PropSet { obj: ValueId, key: ValueId, val: ValueId },
    /// Get a property from a statically known shape.
    ShapeGet { obj: ValueId, shape_id: u32, field_idx: u32 },
    /// Set a property on a statically known shape.
    ShapeSet { obj: ValueId, shape_id: u32, field_idx: u32, val: ValueId },

    // ── Arrays ──
    /// Allocate a new JS array with given elements.
    ArrayNew(Vec<ValueId>),
    /// Get element by numeric index.
    ArrayGet { arr: ValueId, idx: ValueId },
    /// Set element by numeric index.
    ArraySet { arr: ValueId, idx: ValueId, val: ValueId },

    // ── Memory / TypedArrays ──
    /// Load from memory: base pointer, offset value, element size (in bytes), and whether it is a float.
    MemLoad { ptr: ValueId, offset: ValueId, size: u32, is_float: bool },
    /// Store to memory: base pointer, offset value, element size (in bytes), value to store, and whether it is a float.
    MemStore { ptr: ValueId, offset: ValueId, size: u32, val: ValueId, is_float: bool },

    // ── SIMD / Vector Operations ──
    /// Load 128-bit vector: base pointer, offset value.
    VecLoad128 { ptr: ValueId, offset: ValueId },
    /// Store 128-bit vector: base pointer, offset value, vector value to store.
    VecStore128 { ptr: ValueId, offset: ValueId, val: ValueId },
    /// Add two 64-bit float vectors (f64x2).
    VecAdd64x2 { lhs: ValueId, rhs: ValueId },
    /// Add four 32-bit integer vectors (i32x4).
    VecAdd32x4 { lhs: ValueId, rhs: ValueId },

    // ── Functions / Calls ──
    /// Load a reference to a defined function by its FuncId.
    FuncRef(FuncId),
    /// Call a JS value as a function.
    Call { callee: ValueId, this: ValueId, args: Vec<ValueId> },
    /// Return from the current function.
    Return(Option<ValueId>),
    /// Return and call via tail-call optimization.
    ReturnCall { callee: ValueId, this: ValueId, args: Vec<ValueId> },

    // ── Control Flow ──
    /// Unconditional branch.
    Jump(BlockId),
    /// Conditional branch.
    Branch { cond: ValueId, then_bb: BlockId, else_bb: BlockId },
    /// Φ-node: select a value depending on which predecessor we came from.
    Phi(Vec<(BlockId, ValueId)>),

    // ── Exceptions ──
    Throw(ValueId),
    /// Begin a try region; `catch_bb` is entered on exception.
    TryBegin { body_bb: BlockId, catch_bb: BlockId, finally_bb: Option<BlockId>, merge_bb: BlockId },
    TryEnd,

    // ── Host calls ──
    /// Call a host-imported function by name.
    HostCall { name: String, args: Vec<ValueId> },

    // ── Type helpers ──
    /// typeof operator.
    TypeOf(ValueId),
    /// Convert to boolean (truthy check).
    Truthy(ValueId),
    /// Coerce to number.
    ToNumber(ValueId),
    /// Coerce to string.
    ToString(ValueId),
}

// ─── Binary / Unary Operators ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add, Sub, Mul, Div, Rem, Pow,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr, UShr,
    // Comparison
    Eq, StrictEq, Ne, StrictNe, Lt, Le, Gt, Ge,
    // Logical
    And, Or, Nullish,
    // Object
    In, InstanceOf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg, Not, BitNot, TypeOf, Void, Delete,
    PreInc, PreDec, PostInc, PostDec,
}

// ─── BasicBlock ───────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct BasicBlock {
    pub name:  String,
    pub insts: Vec<Instruction>,
    /// Predecessor blocks (for SSA construction).
    pub preds: Vec<BlockId>,
}

// ─── Function ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Function {
    pub name:       String,
    pub params:     Vec<(String, JsType)>,
    pub ret_ty:     JsType,
    pub blocks:     Arena<BasicBlock>,
    pub entry:      BlockId,
    pub values:     Arena<Value>,
    /// Local variable name → slot index
    pub locals:     indexmap::IndexMap<String, u32>,
    /// Number of local slots allocated.
    pub local_count: u32,
}

impl Function {
    pub fn new(name: impl Into<String>, params: Vec<(String, JsType)>) -> Self {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            ..Default::default()
        });
        Self {
            name: name.into(),
            params,
            ret_ty: JsType::Any,
            blocks,
            entry,
            values: Arena::new(),
            locals: indexmap::IndexMap::new(),
            local_count: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct IrModule {
    pub functions: Arena<Function>,
    /// Interned string table (index = string index in Const::String).
    pub strings:   Vec<String>,
    /// The top-level "main" function id.
    pub main:      Option<FuncId>,
    /// Registered object shapes (each entry maps shape_id -> list of field names).
    pub shapes:    Vec<Vec<String>>,
}

impl IrModule {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a shape with given fields and return its shape_id.
    pub fn register_shape(&mut self, fields: Vec<String>) -> u32 {
        if let Some(i) = self.shapes.iter().position(|x| x == &fields) {
            return i as u32;
        }
        let idx = self.shapes.len() as u32;
        self.shapes.push(fields);
        idx
    }

    /// Intern a string and return its index.
    pub fn intern_string(&mut self, s: impl Into<String>) -> u32 {
        let s = s.into();
        if let Some(i) = self.strings.iter().position(|x| x == &s) {
            return i as u32;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s);
        idx
    }
}
