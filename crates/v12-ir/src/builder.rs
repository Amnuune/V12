//! IrBuilder: ergonomic builder API for constructing SSA IR.

use id_arena::Arena;
use crate::{
    BasicBlock, BlockId, Const, FuncId, Function, Instruction, IrModule,
    JsType, Op, Value, ValueId, ValueKind,
};

/// Mutable cursor into a single `Function` being built.
///
/// Usage:
/// ```ignore
/// let mut b = IrBuilder::new(&mut module, func_id);
/// let v = b.const_int(42);
/// b.return_(Some(v));
/// ```
pub struct IrBuilder<'m> {
    pub module:   &'m mut IrModule,
    pub func_id:  FuncId,
    /// The block we are currently appending instructions to.
    pub current:  BlockId,
}

impl<'m> IrBuilder<'m> {
    pub fn new(module: &'m mut IrModule, func_id: FuncId) -> Self {
        let entry = module.functions[func_id].entry;
        Self { module, func_id, current: entry }
    }

    // ── Block management ─────────────────────────────────────────────────────

    /// Create a new (empty) basic block in the current function.
    pub fn new_block(&mut self, name: impl Into<String>) -> BlockId {
        let f = &mut self.module.functions[self.func_id];
        f.blocks.alloc(BasicBlock {
            name: name.into(),
            ..Default::default()
        })
    }

    /// Switch the insertion point to a different block.
    pub fn switch_to(&mut self, block: BlockId) {
        self.current = block;
    }

    // ── Value allocation ─────────────────────────────────────────────────────

    fn alloc_value(&mut self, ty: JsType, kind: ValueKind) -> ValueId {
        let f = &mut self.module.functions[self.func_id];
        f.values.alloc(Value { ty, kind })
    }

    pub fn value_type(&self, val: ValueId) -> JsType {
        self.module.functions[self.func_id].values[val].ty
    }

    // ── Emit an instruction ──────────────────────────────────────────────────

    fn emit(&mut self, op: Op, ty: JsType) -> ValueId {
        let f        = &mut self.module.functions[self.func_id];
        let idx      = f.blocks[self.current].insts.len();
        let val_kind = ValueKind::Inst { block: self.current, index: idx };
        let val_id   = f.values.alloc(Value { ty, kind: val_kind });
        f.blocks[self.current].insts.push(Instruction {
            result: Some(val_id),
            op,
        });
        val_id
    }

    fn emit_void(&mut self, op: Op) {
        let f = &mut self.module.functions[self.func_id];
        f.blocks[self.current].insts.push(Instruction { result: None, op });
    }

    // ── Constants ────────────────────────────────────────────────────────────

    pub fn const_undefined(&mut self) -> ValueId {
        self.emit(Op::Const(Const::Undefined), JsType::Any)
    }
    pub fn const_null(&mut self) -> ValueId {
        self.emit(Op::Const(Const::Null), JsType::Any)
    }
    pub fn const_bool(&mut self, b: bool) -> ValueId {
        self.emit(Op::Const(Const::Bool(b)), JsType::Bool)
    }
    pub fn const_int(&mut self, i: i32) -> ValueId {
        self.emit(Op::Const(Const::Int(i)), JsType::Int)
    }
    pub fn const_float(&mut self, f: f64) -> ValueId {
        self.emit(Op::Const(Const::Float(f)), JsType::Float)
    }
    pub fn const_string(&mut self, s: impl Into<String>) -> ValueId {
        let idx = self.module.intern_string(s);
        self.emit(Op::Const(Const::String(idx)), JsType::Any)
    }

    // ── Binary / Unary ops ───────────────────────────────────────────────────

    pub fn binop(&mut self, op: crate::BinOp, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit(Op::BinOp { op, lhs, rhs }, JsType::Any)
    }

    pub fn unop(&mut self, op: crate::UnOp, val: ValueId) -> ValueId {
        self.emit(Op::UnOp { op, val }, JsType::Any)
    }

    // ── Locals ───────────────────────────────────────────────────────────────

    /// Declare a local variable; returns its slot index.
    pub fn local_new(&mut self, name: impl Into<String>) -> u32 {
        let name = name.into();
        let f    = &mut self.module.functions[self.func_id];
        if let Some(&slot) = f.locals.get(&name) {
            return slot;
        }
        let slot = f.local_count;
        f.local_count += 1;
        f.locals.insert(name.clone(), slot);
        f.blocks[self.current].insts.push(Instruction {
            result: None,
            op: Op::LocalNew { name },
        });
        slot
    }

    pub fn local_get(&mut self, slot: u32) -> ValueId {
        self.emit(Op::LocalGet(slot), JsType::Any)
    }

    pub fn local_set(&mut self, slot: u32, val: ValueId) {
        self.emit_void(Op::LocalSet(slot, val));
    }

    // ── Objects ──────────────────────────────────────────────────────────────

    pub fn object_new(&mut self) -> ValueId {
        self.emit(Op::ObjectNew, JsType::Any)
    }

    pub fn object_shape_new(&mut self, shape_id: u32, args: Vec<ValueId>) -> ValueId {
        self.emit(Op::ObjectShapeNew { shape_id, args }, JsType::ObjectShape(shape_id))
    }

    pub fn prop_get(&mut self, obj: ValueId, key: ValueId) -> ValueId {
        self.emit(Op::PropGet { obj, key }, JsType::Any)
    }

    pub fn prop_set(&mut self, obj: ValueId, key: ValueId, val: ValueId) {
        self.emit_void(Op::PropSet { obj, key, val });
    }

    // ── Memory / SIMD / Vector operations ──

    pub fn mem_load(&mut self, ptr: ValueId, offset: ValueId, size: u32, is_float: bool) -> ValueId {
        let ty = if is_float { JsType::Float } else { JsType::Int };
        self.emit(Op::MemLoad { ptr, offset, size, is_float }, ty)
    }

    pub fn mem_store(&mut self, ptr: ValueId, offset: ValueId, size: u32, val: ValueId, is_float: bool) {
        self.emit_void(Op::MemStore { ptr, offset, size, val, is_float });
    }

    pub fn vec_load128(&mut self, ptr: ValueId, offset: ValueId) -> ValueId {
        self.emit(Op::VecLoad128 { ptr, offset }, JsType::Vector128)
    }

    pub fn vec_store128(&mut self, ptr: ValueId, offset: ValueId, val: ValueId) {
        self.emit_void(Op::VecStore128 { ptr, offset, val });
    }

    pub fn vec_add64x2(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit(Op::VecAdd64x2 { lhs, rhs }, JsType::Vector128)
    }

    pub fn vec_add32x4(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        self.emit(Op::VecAdd32x4 { lhs, rhs }, JsType::Vector128)
    }

    pub fn shape_get(&mut self, obj: ValueId, shape_id: u32, field_idx: u32) -> ValueId {
        self.emit(Op::ShapeGet { obj, shape_id, field_idx }, JsType::Any)
    }

    pub fn shape_set(&mut self, obj: ValueId, shape_id: u32, field_idx: u32, val: ValueId) {
        self.emit_void(Op::ShapeSet { obj, shape_id, field_idx, val });
    }

    // ── Arrays ───────────────────────────────────────────────────────────────

    pub fn array_new(&mut self, elems: Vec<ValueId>) -> ValueId {
        self.emit(Op::ArrayNew(elems), JsType::Any)
    }

    pub fn array_get(&mut self, arr: ValueId, idx: ValueId) -> ValueId {
        self.emit(Op::ArrayGet { arr, idx }, JsType::Any)
    }

    pub fn array_set(&mut self, arr: ValueId, idx: ValueId, val: ValueId) {
        self.emit_void(Op::ArraySet { arr, idx, val });
    }

    // ── Calls ────────────────────────────────────────────────────────────────

    pub fn call(&mut self, callee: ValueId, this: ValueId, args: Vec<ValueId>) -> ValueId {
        self.emit(Op::Call { callee, this, args }, JsType::Any)
    }

    pub fn host_call(&mut self, name: impl Into<String>, args: Vec<ValueId>) -> ValueId {
        self.emit(Op::HostCall { name: name.into(), args }, JsType::Any)
    }

    // ── Control flow ─────────────────────────────────────────────────────────

    pub fn jump(&mut self, target: BlockId) {
        self.emit_void(Op::Jump(target));
    }

    pub fn branch(&mut self, cond: ValueId, then_bb: BlockId, else_bb: BlockId) {
        self.emit_void(Op::Branch { cond, then_bb, else_bb });
    }

    pub fn return_(&mut self, val: Option<ValueId>) {
        self.emit_void(Op::Return(val));
    }

    pub fn return_call(&mut self, callee: ValueId, this: ValueId, args: Vec<ValueId>) {
        self.emit_void(Op::ReturnCall { callee, this, args });
    }

    pub fn throw(&mut self, val: ValueId) {
        self.emit_void(Op::Throw(val));
    }

    pub fn try_begin(&mut self, body_bb: BlockId, catch_bb: BlockId, finally_bb: Option<BlockId>, merge_bb: BlockId) {
        self.emit_void(Op::TryBegin { body_bb, catch_bb, finally_bb, merge_bb });
    }

    // ── Type helpers ─────────────────────────────────────────────────────────

    pub fn typeof_(&mut self, val: ValueId) -> ValueId {
        self.emit(Op::TypeOf(val), JsType::Any)
    }

    pub fn truthy(&mut self, val: ValueId) -> ValueId {
        self.emit(Op::Truthy(val), JsType::Bool)
    }

    pub fn to_number(&mut self, val: ValueId) -> ValueId {
        self.emit(Op::ToNumber(val), JsType::Float)
    }

    pub fn to_string(&mut self, val: ValueId) -> ValueId {
        self.emit(Op::ToString(val), JsType::Any)
    }

    // ── Func ref ─────────────────────────────────────────────────────────────

    pub fn func_ref(&mut self, id: FuncId) -> ValueId {
        self.emit(Op::FuncRef(id), JsType::Any)
    }
}
