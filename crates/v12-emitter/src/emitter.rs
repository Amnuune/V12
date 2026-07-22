//! Emitter: V12 IrModule → WasmGC bytecode.
//!
//! ## Value pre-pass
//! Before any instruction emission, every SSA ValueId is assigned a dedicated
//! Wasm local (externref). This makes cross-block value references trivially
//! correct — `load(vid)` always succeeds.
//!
//! ## Control flow (name-based)
//! The IR builder names blocks consistently (e.g. "for.cond", "while.body",
//! "if.then"). We exploit these names to pick the right Wasm construct:
//!
//!  - "*.cond" or "while.cond" → loop header: emit `block`+`loop`
//!  - Branch from a "*.cond" block:
//!      then_bb  → loop body
//!      else_bb  → loop after  → emit `br_if 1` (exit block) when cond false,
//!                                fall into body, `br 0` at end of update block
//!  - Branch from any other block (if/ternary) → `if / else / end`
//!
//! This pattern produces valid, non-infinite Wasm for all lifted JS constructs.

use anyhow::Result;
use id_arena::Arena;
use indexmap::IndexMap;
use std::collections::HashSet;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function as WasmFunc, FunctionSection,
    ImportSection, Instruction as WI, Module, TypeSection, ValType, EntityType,
    BlockType,
};
use v12_ir::{BasicBlock, BinOp, BlockId, Const, FuncId, IrModule, Op, UnOp, ValueId};

const SHAPE_TYPE_OFFSET: u32 = 13;

pub struct Emitter<'m> {
    module: &'m IrModule,
    pub string_offset: u32,
}

impl<'m> Emitter<'m> {
    pub fn new(module: &'m IrModule) -> Self { Self { module, string_offset: 0 } }
    pub fn with_offset(module: &'m IrModule, string_offset: u32) -> Self { Self { module, string_offset } }

    pub fn emit(&self) -> Result<Vec<u8>> {
        let mut wasm = Module::new();

        // ── Types ─────────────────────────────────────────────────────────────
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::EXTERNREF]);                                       // 0
        types.ty().function([ValType::EXTERNREF], [ValType::EXTERNREF]);                     // 1
        types.ty().function([ValType::EXTERNREF, ValType::EXTERNREF], [ValType::EXTERNREF]); // 2
        types.ty().function([ValType::EXTERNREF,ValType::EXTERNREF,ValType::EXTERNREF],[ValType::EXTERNREF]); // 3
        types.ty().function([ValType::EXTERNREF,ValType::EXTERNREF,ValType::I32],[ValType::EXTERNREF]); // 4
        types.ty().function([ValType::I32], [ValType::EXTERNREF]);                           // 5
        types.ty().function([ValType::F64], [ValType::EXTERNREF]);                           // 6
        types.ty().function([ValType::EXTERNREF], [ValType::I32]);                           // 7
        types.ty().function([], [ValType::I32]);                                             // 8
        types.ty().function([ValType::EXTERNREF, ValType::I32], []);                         // 9
        types.ty().function([ValType::I64], [ValType::EXTERNREF]);                           // 10
        types.ty().function([ValType::EXTERNREF], [ValType::F64]);                           // 11
        types.ty().function([ValType::EXTERNREF], [ValType::I64]);                           // 12


        // Declare dynamic WasmGC shapes
        for shape in &self.module.shapes {
            let mut fields = Vec::new();
            for _ in shape {
                fields.push(wasm_encoder::FieldType {
                    element_type: wasm_encoder::StorageType::Val(ValType::EXTERNREF),
                    mutable: true,
                });
            }
            types.ty().struct_(fields);
        }

        wasm.section(&types);
 
        // ── Imports ───────────────────────────────────────────────────────────
        let rt_list: &[(&str, u32)] = &[
            ("rt_undefined",0),("rt_null",0),("rt_true",0),("rt_false",0),
            ("rt_object_new",0),("rt_array_new",0),
            ("rt_int",5),("rt_float",6),("rt_string",5),("rt_truthy_i32",7),
            ("rt_neg",1),("rt_not",1),("rt_bit_not",1),("rt_typeof",1),
            ("rt_void",1),("rt_delete",1),
            ("rt_pre_inc",1),("rt_pre_dec",1),("rt_post_inc",1),("rt_post_dec",1),
            ("rt_truthy",1),("rt_to_number",1),("rt_to_string",1),
            ("rt_console_log",1),("rt_console_error",1),("rt_console_warn",1),
            ("rt_eval_js",1),("rt_global_get",1),("rt_push_arg",1),
            ("rt_new",1),("rt_js_new",4),("rt_js_await",1),("rt_throw",1),
            ("rt_add",2),("rt_sub",2),("rt_mul",2),("rt_div",2),
            ("rt_rem",2),("rt_pow",2),
            ("rt_bit_and",2),("rt_bit_or",2),("rt_bit_xor",2),
            ("rt_shl",2),("rt_shr",2),("rt_ushr",2),
            ("rt_eq",2),("rt_strict_eq",2),("rt_ne",2),("rt_strict_ne",2),
            ("rt_lt",2),("rt_le",2),("rt_gt",2),("rt_ge",2),
            ("rt_and",2),("rt_or",2),("rt_nullish",2),
            ("rt_in",2),("rt_instanceof",2),
            ("rt_prop_get",2),("rt_array_get",2),("rt_global_set",2),
            ("rt_prop_set",3),("rt_array_set",3),
            ("rt_call",4),
            ("rt_has_exception",8),
            ("rt_get_exception",0),
            ("rt_get_arg",5),
            ("rt_get_this",0),
            ("rt_tail_call_frame",9),
            ("rt_object_spread", 2),
            ("rt_array_spread", 2),
            ("rt_array_push", 2),
            ("rt_get_keys", 1),
            ("rt_get_values", 1),
            ("rt_typed_array_ptr", 7),
            ("rt_to_int32", 7),
            ("rt_val_to_i32", 7),
            ("rt_val_to_f64", 11),
            ("rt_val_to_i64", 12),
            ("rt_bigint_from_i64", 10),
        ];
        let mut imports = ImportSection::new();
        let mut im: IndexMap<&'static str, u32> = IndexMap::new();
        for (i,(n,t)) in rt_list.iter().enumerate() {
            imports.import("v12rt", *n, EntityType::Function(*t));
            im.insert(n, i as u32);
        }
        imports.import(
            "v12rt",
            "memory",
            EntityType::Memory(wasm_encoder::MemoryType {
                minimum: 1,
                maximum: None,
                shared: false,
                memory64: false,
                page_size_log2: None,
            }),
        );
        wasm.section(&imports);
        let n_imp = rt_list.len() as u32;

        // ── Functions / Exports / Code ─────────────────────────────────────────
        let func_ids: Vec<FuncId> = self.module.functions.iter().map(|(id,_)|id).collect();
        let mut fsec = FunctionSection::new();
        for _ in &func_ids { fsec.function(0); }
        wasm.section(&fsec);

        let mut esec = ExportSection::new();
        if let Some(mid) = self.module.main {
            if let Some(p) = func_ids.iter().position(|&id| id==mid) {
                esec.export("__main__", ExportKind::Func, n_imp + p as u32);
            }
        }
        for (i, &fid) in func_ids.iter().enumerate() {
            esec.export(&format!("func_{}", n_imp + i as u32), ExportKind::Func, n_imp + i as u32);
        }
        wasm.section(&esec);

        let mut code = CodeSection::new();
        for &fid in &func_ids {
            code.function(&self.emit_function(fid, &im, &func_ids, n_imp)?);
        }
        wasm.section(&code);
        Ok(wasm.finish())
    }

    // ─── Emit one function ────────────────────────────────────────────────────

    fn emit_function(
        &self, func_id: FuncId,
        im: &IndexMap<&'static str, u32>,
        func_ids: &[FuncId], n_imp: u32,
    ) -> Result<WasmFunc> {
        let func = &self.module.functions[func_id];

        // Pre-pass: assign every result ValueId a Wasm local.
        let mut vl: IndexMap<ValueId, u32> = IndexMap::new();
        let mut next_externref = func.local_count;

        // Group vector values to allocate them to ValType::V128
        let mut v128_indices = Vec::new();
        for (_, blk) in func.blocks.iter() {
            for inst in &blk.insts {
                if let Some(vid) = inst.result {
                    if func.values[vid].ty == v12_ir::JsType::Vector128 {
                        v128_indices.push(vid);
                    }
                }
            }
        }

        let num_v128 = v128_indices.len() as u32;

        // Assign externrefs first
        for (_, blk) in func.blocks.iter() {
            for inst in &blk.insts {
                if let Some(vid) = inst.result {
                    if func.values[vid].ty != v12_ir::JsType::Vector128 {
                        vl.insert(vid, next_externref);
                        next_externref += 1;
                    }
                }
            }
        }

        // Assign v128 next, starting right after the last externref
        let mut next_v128 = next_externref;
        for vid in v128_indices {
            vl.insert(vid, next_v128);
            next_v128 += 1;
        }

        let mut all_locals: Vec<(u32, ValType)> = Vec::new();
        if next_externref > 0 {
            all_locals.push((next_externref, ValType::EXTERNREF));
        }
        if num_v128 > 0 {
            all_locals.push((num_v128, ValType::V128));
        }

        let mut f = WasmFunc::new(all_locals);

        // Initialize parameter locals from the active call frame on function entry.
        for idx in 0..func.params.len() {
            f.instruction(&WI::I32Const(idx as i32));
            f.instruction(&WI::Call(im["rt_get_arg"]));
            f.instruction(&WI::LocalSet(idx as u32));
        }

        // Build block ordering.
        if func.blocks.iter().next().is_none() {
            f.instruction(&WI::Call(im["rt_undefined"]));
            f.instruction(&WI::Return);
            f.instruction(&WI::End);
            return Ok(f);
        }

        let entry_id = func.blocks.iter().next().unwrap().0;
        let ordered_ids = Self::order_blocks_dfs(&func.blocks, entry_id);
        let blocks: Vec<(BlockId, &BasicBlock)> = ordered_ids.into_iter()
            .map(|bid| (bid, &func.blocks[bid]))
            .collect();

        let bidx: IndexMap<BlockId, usize> = blocks.iter()
            .enumerate()
            .map(|(i,(id,_))| (*id, i))
            .collect();

        // ── Determine loop structure by name ──────────────────────────────────
        // A block is a loop header if its name ends with ".cond".
        // We record (cond_idx, body_idx, update_idx, after_idx) tuples.
        // For "while": cond → body → (cond back-edge). after follows cond.
        // For "for":   cond → body → update → (cond back-edge). after follows.

        // Which block indices are loop headers?
        let loop_cond_idxs: HashSet<usize> = blocks.iter()
            .enumerate()
            .filter(|(_,(_, blk))| blk.name.ends_with(".cond"))
            .map(|(i,_)| i)
            .collect();

        // Consumed = emitted inline (if-arm or loop body), skip in outer walk.
        let mut consumed: HashSet<usize> = HashSet::new();

        // Loop stack: (lh_idx, lh_depth, cond_bb, after_bb, update_bb). Used to compute br depth.
        // Each loop opens: block (for exit) + loop (for continue).
        let mut loop_stack: Vec<(usize, u32, BlockId, BlockId, Option<BlockId>)> = Vec::new();
        let mut nesting_depth = 0u32;
        let mut active_try_stack: Vec<u32> = Vec::new();

        for (i, (_bid, blk)) in blocks.iter().enumerate() {
            if consumed.contains(&i) { continue; }

            // Close loops whose after block we are about to emit.
            // This must happen BEFORE we emit this block's content.
            loop {
                let Some(&(lh_idx, _, _, _, _)) = loop_stack.last() else { break };
                let (_, cond_blk) = &blocks[lh_idx];
                let after_idx = if let Some(bt) = Self::find_term(cond_blk) {
                    match &cond_blk.insts[bt].op {
                        Op::Branch { else_bb, .. } => bidx.get(else_bb).copied(),
                        _ => None,
                    }
                } else { None };
                if after_idx == Some(i) {
                    f.instruction(&WI::End); // end loop
                    f.instruction(&WI::End); // end block
                    loop_stack.pop();
                    nesting_depth -= 2;
                } else { break; }
            }

            // Open loop for loop headers.
            if loop_cond_idxs.contains(&i) {
                // block (exit target) + loop (continue target)
                f.instruction(&WI::Block(BlockType::Empty));
                f.instruction(&WI::Loop(BlockType::Empty));

                let cond_bb = blocks[i].0;
                let cond_blk = blocks[i].1;
                let after_bb = if let Some(bt) = Self::find_term(cond_blk) {
                    match &cond_blk.insts[bt].op {
                        Op::Branch { else_bb, .. } => *else_bb,
                        _ => cond_bb, // fallback
                    }
                } else { cond_bb };

                let mut update_bb = None;
                for (bid, bblk) in &blocks {
                    let bid = *bid;
                    if let Some(ti) = Self::find_term(bblk) {
                        if let Op::Jump(t) = &bblk.insts[ti].op {
                            if *t == cond_bb {
                                update_bb = Some(bid);
                                break;
                            }
                        }
                    }
                }

                loop_stack.push((i, nesting_depth + 2, cond_bb, after_bb, update_bb));
                nesting_depth += 2;
            }

            // Emit body instructions.
            let term = Self::find_term(blk);
            let end = term.unwrap_or(blk.insts.len());
            for inst in &blk.insts[..end] {
                self.emit_inst(
                    &inst.op, inst.result, &mut f, &vl, im, func_ids, n_imp,
                    &mut nesting_depth, &mut active_try_stack, &mut consumed,
                    &bidx, &blocks, func_id, &loop_stack
                )?;
            }

            // Emit terminator.
            if let Some(ti) = term {
                match &blk.insts[ti].op {
                    Op::Return(val) => {
                        self.load_or_undef(*val, &mut f, &vl, im);
                        f.instruction(&WI::Return);
                    }
                    Op::ReturnCall { callee, this, args } => {
                        let mut static_callee = None;
                        if let Some(callee_fid) = self.get_funcref(func_id, *callee) {
                            static_callee = Some(callee_fid);
                        }
                        if let Some(fid) = static_callee {
                            for a in args {
                                self.load(&mut f, &vl, im, *a);
                                f.instruction(&WI::Call(im["rt_push_arg"]));
                                f.instruction(&WI::Drop);
                            }
                            self.load(&mut f, &vl, im, *this);
                            f.instruction(&WI::I32Const(args.len() as i32));
                            f.instruction(&WI::Call(im["rt_tail_call_frame"]));
                            let idx = func_ids.iter().position(|x| *x == fid).unwrap() as u32;
                            f.instruction(&WI::ReturnCall(n_imp + idx));
                        } else {
                            for a in args {
                                self.load(&mut f, &vl, im, *a);
                                f.instruction(&WI::Call(im["rt_push_arg"]));
                                f.instruction(&WI::Drop);
                            }
                            self.load(&mut f, &vl, im, *callee);
                            self.load(&mut f, &vl, im, *this);
                            f.instruction(&WI::I32Const(args.len() as i32));
                            f.instruction(&WI::Call(im["rt_call"]));
                            f.instruction(&WI::Return);
                        }
                    }

                    Op::Jump(target) => {
                        self.emit_jump(*target, &mut f, nesting_depth, &loop_stack, &vl, im, &blocks, func_id)?;
                    }

                    Op::Branch { cond, then_bb, else_bb } => {
                        let then_idx = bidx.get(then_bb).copied();
                        let else_idx = bidx.get(else_bb).copied();

                        if loop_cond_idxs.contains(&i) {
                            // Loop condition block: emit exit check.
                            // Pattern: if NOT truthy → break (br 1 = exit block)
                            //          otherwise fall into body.
                            self.load_or_undef(Some(*cond), &mut f, &vl, im);
                            f.instruction(&WI::Call(im["rt_truthy_i32"]));
                            f.instruction(&WI::I32Eqz);
                            f.instruction(&WI::BrIf(1)); // exit block

                            // Body blocks: emit them inline.
                            // Find all blocks from then_idx up to (not including) else_idx.
                            let body_start = then_idx.unwrap_or(i+1);
                            let body_end   = else_idx.unwrap_or(blocks.len());

                            for j in body_start..body_end {
                                if consumed.contains(&j) { continue; }
                                let (_, bblk) = &blocks[j];
                                let bterm = Self::find_term(bblk);
                                let bend  = bterm.unwrap_or(bblk.insts.len());

                                for inst in &bblk.insts[..bend] {
                                    self.emit_inst(
                                        &inst.op, inst.result, &mut f, &vl, im, func_ids, n_imp,
                                        &mut nesting_depth, &mut active_try_stack, &mut consumed,
                                        &bidx, &blocks, func_id, &loop_stack
                                    )?;
                                }

                                // Handle terminator of body/update blocks.
                                if let Some(bt) = bterm {
                                    match &bblk.insts[bt].op {
                                         Op::Return(val) => {
                                             self.load_or_undef(*val, &mut f, &vl, im);
                                             f.instruction(&WI::Return);
                                         }
                                         Op::ReturnCall { callee, this, args } => {
                                             let mut static_callee = None;
                                             if let Some(callee_fid) = self.get_funcref(func_id, *callee) {
                                                 static_callee = Some(callee_fid);
                                             }
                                             if let Some(fid) = static_callee {
                                                 for a in args {
                                                     self.load(&mut f, &vl, im, *a);
                                                     f.instruction(&WI::Call(im["rt_push_arg"]));
                                                     f.instruction(&WI::Drop);
                                                 }
                                                 self.load(&mut f, &vl, im, *this);
                                                 f.instruction(&WI::I32Const(args.len() as i32));
                                                 f.instruction(&WI::Call(im["rt_tail_call_frame"]));
                                                 let idx = func_ids.iter().position(|x| *x == fid).unwrap() as u32;
                                                 f.instruction(&WI::ReturnCall(n_imp + idx));
                                             } else {
                                                 for a in args {
                                                     self.load(&mut f, &vl, im, *a);
                                                     f.instruction(&WI::Call(im["rt_push_arg"]));
                                                     f.instruction(&WI::Drop);
                                                 }
                                                 self.load(&mut f, &vl, im, *callee);
                                                 self.load(&mut f, &vl, im, *this);
                                                 f.instruction(&WI::I32Const(args.len() as i32));
                                                 f.instruction(&WI::Call(im["rt_call"]));
                                                 f.instruction(&WI::Return);
                                             }
                                         }
                                        Op::Jump(target) => {
                                            self.emit_jump(*target, &mut f, nesting_depth, &loop_stack, &vl, im, &blocks, func_id)?;
                                        }
                                        Op::Branch { cond: c2, then_bb: t2, else_bb: e2 } => {
                                            // Nested if inside loop body — emit if/else/end.
                                            self.emit_nested_branch(
                                                *c2, t2, e2, &mut f, &vl, im, &bidx, &blocks,
                                                func_ids, n_imp, &mut consumed,
                                                &mut nesting_depth, &mut active_try_stack,
                                                func_id,
                                                &loop_stack,
                                            )?;
                                        }
                                        _ => {}
                                    }
                                }
                                consumed.insert(j);
                            }

                            // Mark else (after) block as not consumed — it follows naturally.
                            // Close: br 0 to loop top is already emitted by the update block's Jump.

                        } else {
                            // Non-loop branch: standard if/else/end.
                            self.emit_nested_branch(
                                *cond, then_bb, else_bb, &mut f, &vl, im, &bidx, &blocks,
                                func_ids, n_imp, &mut consumed,
                                &mut nesting_depth, &mut active_try_stack,
                                func_id,
                                &loop_stack,
                            )?;
                        }
                    }

                    Op::Throw(val) => {
                        self.load_or_undef(Some(*val), &mut f, &vl, im);
                        f.instruction(&WI::Call(im["rt_throw"]));
                        f.instruction(&WI::Drop);
                        if let Some(&try_depth) = active_try_stack.last() {
                            let br_label = nesting_depth - try_depth - 2;
                            f.instruction(&WI::Br(br_label));
                        } else {
                            f.instruction(&WI::Unreachable);
                        }
                    }

                    _ => {}
                }
            }
        }

        // Unclosed loops (safety).
        for _ in &loop_stack {
            f.instruction(&WI::End); // end loop
            f.instruction(&WI::End); // end block
        }

        f.instruction(&WI::Call(im["rt_undefined"]));
        f.instruction(&WI::Return);
        f.instruction(&WI::End);
        Ok(f)
    }

    // ─── Emit a non-loop if/else/end branch ──────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn emit_nested_branch(
        &self,
        cond:     ValueId,
        then_bb:  &BlockId,
        else_bb:  &BlockId,
        f:        &mut WasmFunc,
        vl:       &IndexMap<ValueId, u32>,
        im:       &IndexMap<&'static str, u32>,
        bidx:     &IndexMap<BlockId, usize>,
        blocks:   &[(BlockId, &BasicBlock)],
        func_ids: &[FuncId],
        n_imp:    u32,
        consumed: &mut HashSet<usize>,
        nesting_depth: &mut u32,
        active_try_stack: &mut Vec<u32>,
        func_id: FuncId,
        loop_stack: &[(usize, u32, BlockId, BlockId, Option<BlockId>)],
    ) -> Result<()> {
        self.load_or_undef(Some(cond), f, vl, im);
        f.instruction(&WI::Call(im["rt_truthy_i32"]));
        f.instruction(&WI::If(BlockType::Empty));
        *nesting_depth += 1;

        // then arm
        if let Some(&ti) = bidx.get(then_bb) {
            let (_, tblk) = &blocks[ti];
            self.emit_arm_block(tblk, f, vl, im, bidx, blocks, func_ids, n_imp, nesting_depth, active_try_stack, consumed, func_id, loop_stack)?;
            consumed.insert(ti);
        }

        f.instruction(&WI::Else);

        // else arm
        if let Some(&ei) = bidx.get(else_bb) {
            let (_, eblk) = &blocks[ei];
            self.emit_arm_block(eblk, f, vl, im, bidx, blocks, func_ids, n_imp, nesting_depth, active_try_stack, consumed, func_id, loop_stack)?;
            consumed.insert(ei);
        }

        f.instruction(&WI::End);
        *nesting_depth -= 1;
        Ok(())
    }

    // ─── Emit block instructions for one if/else arm ─────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn emit_arm_block(
        &self,
        blk:      &BasicBlock,
        f:        &mut WasmFunc,
        vl:       &IndexMap<ValueId, u32>,
        im:       &IndexMap<&'static str, u32>,
        bidx:     &IndexMap<BlockId, usize>,
        blocks:   &[(BlockId, &BasicBlock)],
        func_ids: &[FuncId],
        n_imp:    u32,
        nesting_depth: &mut u32,
        active_try_stack: &mut Vec<u32>,
        consumed: &mut HashSet<usize>,
        func_id: FuncId,
        loop_stack: &[(usize, u32, BlockId, BlockId, Option<BlockId>)],
    ) -> Result<()> {
        let term = Self::find_term(blk);
        let end  = term.unwrap_or(blk.insts.len());

        for inst in &blk.insts[..end] {
            self.emit_inst(
                &inst.op, inst.result, f, vl, im, func_ids, n_imp,
                nesting_depth, active_try_stack, consumed,
                bidx, blocks, func_id, loop_stack
            )?;
        }

        if let Some(ti) = term {
            match &blk.insts[ti].op {
                Op::Return(val) => {
                    self.load_or_undef(*val, f, vl, im);
                    f.instruction(&WI::Return);
                }
                Op::ReturnCall { callee, this, args } => {
                    let mut static_callee = None;
                    if let Some(callee_fid) = self.get_funcref(func_id, *callee) {
                        static_callee = Some(callee_fid);
                    }
                    if let Some(fid) = static_callee {
                        for a in args {
                            self.load(f, vl, im, *a);
                            f.instruction(&WI::Call(im["rt_push_arg"]));
                            f.instruction(&WI::Drop);
                        }
                        self.load(f, vl, im, *this);
                        f.instruction(&WI::I32Const(args.len() as i32));
                        f.instruction(&WI::Call(im["rt_tail_call_frame"]));
                        let idx = func_ids.iter().position(|x| *x == fid).unwrap() as u32;
                        f.instruction(&WI::ReturnCall(n_imp + idx));
                    } else {
                        for a in args {
                            self.load(f, vl, im, *a);
                            f.instruction(&WI::Call(im["rt_push_arg"]));
                            f.instruction(&WI::Drop);
                        }
                        self.load(f, vl, im, *callee);
                        self.load(f, vl, im, *this);
                        f.instruction(&WI::I32Const(args.len() as i32));
                        f.instruction(&WI::Call(im["rt_call"]));
                        f.instruction(&WI::Return);
                    }
                }
                Op::Branch { cond, then_bb, else_bb } => {
                    // Nested if/else-if: emit recursively.
                    let mut consumed_dummy: HashSet<usize> = HashSet::new();
                    self.emit_nested_branch(
                        *cond, then_bb, else_bb,
                        f, vl, im, bidx, blocks, func_ids, n_imp,
                        &mut consumed_dummy,
                        nesting_depth, active_try_stack,
                        func_id,
                        loop_stack,
                    )?;
                }
                Op::Jump(target) => {
                    self.emit_jump(*target, f, *nesting_depth, loop_stack, vl, im, blocks, func_id)?;
                }
                Op::Throw(val) => {
                    self.load_or_undef(Some(*val), f, vl, im);
                    f.instruction(&WI::Call(im["rt_throw"]));
                    f.instruction(&WI::Drop);
                    if let Some(&try_depth) = active_try_stack.last() {
                        let br_label = *nesting_depth - try_depth - 2;
                        f.instruction(&WI::Br(br_label));
                    } else {
                        f.instruction(&WI::Unreachable);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ─── Emit a non-terminator instruction ───────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn emit_inst(
        &self,
        op: &Op, result: Option<ValueId>,
        f: &mut WasmFunc,
        vl: &IndexMap<ValueId, u32>,
        im: &IndexMap<&'static str, u32>,
        func_ids: &[FuncId], n_imp: u32,
        nesting_depth: &mut u32,
        active_try_stack: &mut Vec<u32>,
        consumed: &mut HashSet<usize>,
        bidx: &IndexMap<BlockId, usize>,
        blocks: &[(BlockId, &BasicBlock)],
        func_id: FuncId,
        loop_stack: &[(usize, u32, BlockId, BlockId, Option<BlockId>)],
    ) -> Result<()> {
        let pushed = match op {
            Op::Const(c) => match c {
                Const::Undefined  => { f.instruction(&WI::Call(im["rt_undefined"])); true }
                Const::Null       => { f.instruction(&WI::Call(im["rt_null"])); true }
                Const::Bool(b)    => { f.instruction(&WI::Call(if *b {im["rt_true"]} else {im["rt_false"]})); true }
                Const::Int(i)     => { f.instruction(&WI::I32Const(*i)); f.instruction(&WI::Call(im["rt_int"])); true }
                Const::Float(fv)  => { f.instruction(&WI::F64Const(*fv)); f.instruction(&WI::Call(im["rt_float"])); true }
                Const::String(ix) => { f.instruction(&WI::I32Const((*ix + self.string_offset) as i32)); f.instruction(&WI::Call(im["rt_string"])); true }
            },
            Op::LocalNew { .. } => false,
            Op::LocalGet(s)     => { f.instruction(&WI::LocalGet(*s)); true }
            Op::LocalSet(s, v)  => { self.load(f, vl, im, *v); f.instruction(&WI::LocalSet(*s)); false }
            Op::BinOp{op:b,lhs,rhs} => {
                self.load(f, vl, im, *lhs); self.load(f, vl, im, *rhs);
                f.instruction(&WI::Call(im[binop(*b)])); true
            }
            Op::UnOp{op:u,val}  => { self.load(f, vl, im, *val); f.instruction(&WI::Call(im[unop(*u)])); true }
            Op::Truthy(v)       => { self.load(f, vl, im, *v); f.instruction(&WI::Call(im["rt_truthy"])); true }
            Op::TypeOf(v)       => { self.load(f, vl, im, *v); f.instruction(&WI::Call(im["rt_typeof"])); true }
            Op::ToNumber(v)     => { self.load(f, vl, im, *v); f.instruction(&WI::Call(im["rt_to_number"])); true }
            Op::ToString(v)     => { self.load(f, vl, im, *v); f.instruction(&WI::Call(im["rt_to_string"])); true }
            Op::ObjectNew       => { f.instruction(&WI::Call(im["rt_object_new"])); true }
            Op::ObjectShapeNew { shape_id, args } => {
                for a in args {
                    self.load(f, vl, im, *a);
                }
                f.instruction(&WI::StructNew(SHAPE_TYPE_OFFSET + shape_id));
                f.instruction(&WI::ExternConvertAny);
                true
            }
            Op::PropGet{obj,key}=> {
                let mut shape_opt = None;
                if let Some(Const::String(ix)) = self.const_val(func_id, *key) {
                    let prop_name = &self.module.strings[ix as usize];
                    for (sid, shape) in self.module.shapes.iter().enumerate() {
                        if let Some(field_idx) = shape.iter().position(|f| f == prop_name) {
                            shape_opt = Some((sid as u32, field_idx as u32));
                            break;
                        }
                    }
                }

                if let Some((shape_id, field_idx)) = shape_opt {
                    self.load(f, vl, im, *obj);
                    f.instruction(&WI::AnyConvertExtern);
                    f.instruction(&WI::RefTestNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                    f.instruction(&WI::If(BlockType::Result(ValType::EXTERNREF)));
                    self.load(f, vl, im, *obj);
                    f.instruction(&WI::AnyConvertExtern);
                    f.instruction(&WI::RefCastNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                    f.instruction(&WI::StructGet { struct_type_index: SHAPE_TYPE_OFFSET + shape_id, field_index: field_idx });
                    f.instruction(&WI::Else);
                    self.load(f, vl, im, *obj);
                    self.load(f, vl, im, *key);
                    f.instruction(&WI::Call(im["rt_prop_get"]));
                    f.instruction(&WI::End);
                    true
                } else {
                    self.load(f, vl, im, *obj);
                    self.load(f, vl, im, *key);
                    f.instruction(&WI::Call(im["rt_prop_get"]));
                    true
                }
            }
            Op::PropSet{obj,key,val} => {
                let mut shape_opt = None;
                if let Some(Const::String(ix)) = self.const_val(func_id, *key) {
                    let prop_name = &self.module.strings[ix as usize];
                    for (sid, shape) in self.module.shapes.iter().enumerate() {
                        if let Some(field_idx) = shape.iter().position(|f| f == prop_name) {
                            shape_opt = Some((sid as u32, field_idx as u32));
                            break;
                        }
                    }
                }

                if let Some((shape_id, field_idx)) = shape_opt {
                    self.load(f, vl, im, *obj);
                    f.instruction(&WI::AnyConvertExtern);
                    f.instruction(&WI::RefTestNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                    f.instruction(&WI::If(BlockType::Empty));
                    self.load(f, vl, im, *obj);
                    f.instruction(&WI::AnyConvertExtern);
                    f.instruction(&WI::RefCastNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                    self.load(f, vl, im, *val);
                    f.instruction(&WI::StructSet { struct_type_index: SHAPE_TYPE_OFFSET + shape_id, field_index: field_idx });
                    f.instruction(&WI::Else);
                    self.load(f, vl, im, *obj);
                    self.load(f, vl, im, *key);
                    self.load(f, vl, im, *val);
                    f.instruction(&WI::Call(im["rt_prop_set"]));
                    f.instruction(&WI::Drop);
                    f.instruction(&WI::End);
                } else {
                    self.load(f, vl, im, *obj);
                    self.load(f, vl, im, *key);
                    self.load(f, vl, im, *val);
                    f.instruction(&WI::Call(im["rt_prop_set"]));
                    f.instruction(&WI::Drop);
                }
                false
            }
            Op::ShapeGet { obj, shape_id, field_idx } => {
                self.load(f, vl, im, *obj);
                f.instruction(&WI::AnyConvertExtern);
                f.instruction(&WI::RefCastNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                f.instruction(&WI::StructGet { struct_type_index: SHAPE_TYPE_OFFSET + shape_id, field_index: *field_idx });
                true
            }
            Op::ShapeSet { obj, shape_id, field_idx, val } => {
                self.load(f, vl, im, *obj);
                f.instruction(&WI::AnyConvertExtern);
                f.instruction(&WI::RefCastNonNull(wasm_encoder::HeapType::Concrete(SHAPE_TYPE_OFFSET + shape_id)));
                self.load(f, vl, im, *val);
                f.instruction(&WI::StructSet { struct_type_index: SHAPE_TYPE_OFFSET + shape_id, field_index: *field_idx });
                false
            }
            Op::ArrayNew(elems) => {
                f.instruction(&WI::Call(im["rt_array_new"]));
                if let Some(vid) = result {
                    let l = vl[&vid];
                    f.instruction(&WI::LocalSet(l));
                    for (idx, eid) in elems.iter().enumerate() {
                        f.instruction(&WI::LocalGet(l));
                        f.instruction(&WI::I32Const(idx as i32));
                        f.instruction(&WI::Call(im["rt_int"]));
                        self.load(f, vl, im, *eid);
                        f.instruction(&WI::Call(im["rt_array_set"]));
                        f.instruction(&WI::Drop);
                    }
                    f.instruction(&WI::LocalGet(l));
                    true
                } else { f.instruction(&WI::Drop); false }
            }
            Op::ArrayGet{arr,idx}  => { self.load(f,vl,im,*arr); self.load(f,vl,im,*idx); f.instruction(&WI::Call(im["rt_array_get"])); true }
            Op::ArraySet{arr,idx,val} => {
                self.load(f,vl,im,*arr); self.load(f,vl,im,*idx); self.load(f,vl,im,*val);
                f.instruction(&WI::Call(im["rt_array_set"]));
                if result.is_none() { f.instruction(&WI::Drop); false } else { true }
            }
            Op::MemLoad { ptr, offset, size, is_float } => {
                self.load(f, vl, im, *ptr);
                f.instruction(&WI::Call(im["rt_typed_array_ptr"]));
                self.load(f, vl, im, *offset);
                f.instruction(&WI::Call(im["rt_to_int32"]));
                if *size > 1 {
                    f.instruction(&WI::I32Const(*size as i32));
                    f.instruction(&WI::I32Mul);
                }
                f.instruction(&WI::I32Add);
                match (*size, *is_float) {
                    (1, false) => {
                        f.instruction(&WI::I32Load8U(wasm_encoder::MemArg { offset: 0, align: 0, memory_index: 0 }));
                        f.instruction(&WI::Call(im["rt_int"]));
                    }
                    (2, false) => {
                        f.instruction(&WI::I32Load16U(wasm_encoder::MemArg { offset: 0, align: 1, memory_index: 0 }));
                        f.instruction(&WI::Call(im["rt_int"]));
                    }
                    (4, false) => {
                        f.instruction(&WI::I32Load(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
                        f.instruction(&WI::Call(im["rt_int"]));
                    }
                    (4, true) => {
                        f.instruction(&WI::F32Load(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
                        f.instruction(&WI::F64PromoteF32);
                        f.instruction(&WI::Call(im["rt_float"]));
                    }
                    (8, false) => {
                        f.instruction(&WI::I64Load(wasm_encoder::MemArg { offset: 0, align: 3, memory_index: 0 }));
                        f.instruction(&WI::Call(im["rt_bigint_from_i64"]));
                    }
                    (8, true) => {
                        f.instruction(&WI::F64Load(wasm_encoder::MemArg { offset: 0, align: 3, memory_index: 0 }));
                        f.instruction(&WI::Call(im["rt_float"]));
                    }
                    _ => {
                        f.instruction(&WI::Call(im["rt_undefined"]));
                    }
                }
                true
            }
            Op::MemStore { ptr, offset, size, val, is_float } => {
                self.load(f, vl, im, *ptr);
                f.instruction(&WI::Call(im["rt_typed_array_ptr"]));
                self.load(f, vl, im, *offset);
                f.instruction(&WI::Call(im["rt_to_int32"]));
                if *size > 1 {
                    f.instruction(&WI::I32Const(*size as i32));
                    f.instruction(&WI::I32Mul);
                }
                f.instruction(&WI::I32Add);
                match (*size, *is_float) {
                    (1, false) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_i32"]));
                        f.instruction(&WI::I32Store8(wasm_encoder::MemArg { offset: 0, align: 0, memory_index: 0 }));
                    }
                    (2, false) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_i32"]));
                        f.instruction(&WI::I32Store16(wasm_encoder::MemArg { offset: 0, align: 1, memory_index: 0 }));
                    }
                    (4, false) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_i32"]));
                        f.instruction(&WI::I32Store(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
                    }
                    (4, true) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_f64"]));
                        f.instruction(&WI::F32DemoteF64);
                        f.instruction(&WI::F32Store(wasm_encoder::MemArg { offset: 0, align: 2, memory_index: 0 }));
                    }
                    (8, false) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_i64"]));
                        f.instruction(&WI::I64Store(wasm_encoder::MemArg { offset: 0, align: 3, memory_index: 0 }));
                    }
                    (8, true) => {
                        self.load(f, vl, im, *val);
                        f.instruction(&WI::Call(im["rt_val_to_f64"]));
                        f.instruction(&WI::F64Store(wasm_encoder::MemArg { offset: 0, align: 3, memory_index: 0 }));
                    }
                    _ => {}
                }
                false
            }
            Op::VecLoad128 { ptr, offset } => {
                self.load(f, vl, im, *ptr);
                f.instruction(&WI::Call(im["rt_typed_array_ptr"]));
                self.load(f, vl, im, *offset);
                f.instruction(&WI::Call(im["rt_to_int32"]));
                let ty = self.module.functions[func_id].values[*ptr].ty;
                let size = match ty {
                    v12_ir::JsType::TypedArray { element_size, .. } => element_size,
                    _ => 8,
                };
                if size > 1 {
                    f.instruction(&WI::I32Const(size as i32));
                    f.instruction(&WI::I32Mul);
                }
                f.instruction(&WI::I32Add);
                f.instruction(&WI::V128Load(wasm_encoder::MemArg { offset: 0, align: 4, memory_index: 0 }));
                true
            }
            Op::VecStore128 { ptr, offset, val } => {
                self.load(f, vl, im, *ptr);
                f.instruction(&WI::Call(im["rt_typed_array_ptr"]));
                self.load(f, vl, im, *offset);
                f.instruction(&WI::Call(im["rt_to_int32"]));
                let ty = self.module.functions[func_id].values[*ptr].ty;
                let size = match ty {
                    v12_ir::JsType::TypedArray { element_size, .. } => element_size,
                    _ => 8,
                };
                if size > 1 {
                    f.instruction(&WI::I32Const(size as i32));
                    f.instruction(&WI::I32Mul);
                }
                f.instruction(&WI::I32Add);
                self.load(f, vl, im, *val);
                f.instruction(&WI::V128Store(wasm_encoder::MemArg { offset: 0, align: 4, memory_index: 0 }));
                false
            }
            Op::VecAdd64x2 { lhs, rhs } => {
                self.load(f, vl, im, *lhs);
                self.load(f, vl, im, *rhs);
                f.instruction(&WI::F64x2Add);
                true
            }
            Op::VecAdd32x4 { lhs, rhs } => {
                self.load(f, vl, im, *lhs);
                self.load(f, vl, im, *rhs);
                f.instruction(&WI::I32x4Add);
                true
            }
            Op::Call{callee,this,args} => {
                for a in args { self.load(f,vl,im,*a); f.instruction(&WI::Call(im["rt_push_arg"])); f.instruction(&WI::Drop); }
                self.load(f,vl,im,*callee); self.load(f,vl,im,*this);
                f.instruction(&WI::I32Const(args.len() as i32));
                f.instruction(&WI::Call(im["rt_call"])); true
            }
            Op::HostCall{name,args} => {
                if *name == "js_new" {
                    if args.is_empty() {
                        f.instruction(&WI::Call(im["rt_undefined"]));
                        f.instruction(&WI::Call(im["rt_undefined"]));
                        f.instruction(&WI::I32Const(0));
                        f.instruction(&WI::Call(im["rt_js_new"]));
                    } else {
                        let ctor = args[0];
                        let ctor_args = &args[1..];
                        for a in ctor_args {
                            self.load(f, vl, im, *a);
                            f.instruction(&WI::Call(im["rt_push_arg"]));
                            f.instruction(&WI::Drop);
                        }
                        self.load(f, vl, im, ctor);
                        f.instruction(&WI::Call(im["rt_undefined"]));
                        f.instruction(&WI::I32Const(ctor_args.len() as i32));
                        f.instruction(&WI::Call(im["rt_js_new"]));
                    }
                } else {
                    let rn = hcall(name);
                    match args.len() {
                        0 => { f.instruction(&WI::Call(im[rn])); }
                        1 => { self.load(f,vl,im,args[0]); f.instruction(&WI::Call(im[rn])); }
                        2 => { self.load(f,vl,im,args[0]); self.load(f,vl,im,args[1]); f.instruction(&WI::Call(im[rn])); }
                        _ => {
                            for a in args { self.load(f,vl,im,*a); f.instruction(&WI::Call(im["rt_push_arg"])); f.instruction(&WI::Drop); }
                            f.instruction(&WI::Call(im["rt_undefined"]));
                            f.instruction(&WI::Call(im["rt_undefined"]));
                            f.instruction(&WI::I32Const(args.len() as i32));
                            f.instruction(&WI::Call(im["rt_call"]));
                        }
                    }
                }
                true
            }
            Op::FuncRef(fid) => {
                let p = func_ids.iter().position(|&id| id==*fid).unwrap_or(0);
                f.instruction(&WI::I32Const((n_imp+p as u32) as i32));
                f.instruction(&WI::Call(im["rt_int"])); true
            }
            Op::TryBegin { body_bb, catch_bb, merge_bb, .. } => {
                let try_depth = *nesting_depth;
                f.instruction(&WI::Block(BlockType::Empty)); // Outer wrapper
                f.instruction(&WI::Block(BlockType::Empty)); // Catch wrapper
                f.instruction(&WI::Block(BlockType::Empty)); // Try wrapper
                *nesting_depth += 3;
                active_try_stack.push(try_depth);

                let loop_cond_idxs: HashSet<usize> = blocks.iter()
                    .enumerate()
                    .filter(|(_,(_, blk))| blk.name.ends_with(".cond"))
                    .map(|(i,_)| i)
                    .collect();

                let mut lstack = loop_stack.to_vec();

                self.emit_try_body_blocks(
                    *body_bb, f, vl, im, bidx, blocks, func_ids, n_imp,
                    nesting_depth, active_try_stack, consumed, func_id, &mut lstack,
                    *merge_bb, *catch_bb, &loop_cond_idxs
                )?;
                
                active_try_stack.pop();
                f.instruction(&WI::End); // End of Try wrapper block
                *nesting_depth -= 1;
                
                f.instruction(&WI::Br(1)); // Normal completion: exit Outer wrapper block
                
                f.instruction(&WI::End); // End of Catch wrapper block
                *nesting_depth -= 1;
                
                self.emit_try_body_blocks(
                    *catch_bb, f, vl, im, bidx, blocks, func_ids, n_imp,
                    nesting_depth, active_try_stack, consumed, func_id, &mut lstack,
                    *merge_bb, *catch_bb, &loop_cond_idxs
                )?;
                
                f.instruction(&WI::End); // End of Outer wrapper block
                *nesting_depth -= 1;
                
                false
            }
            Op::TryEnd => false,
            _ => false,
        };

        if pushed {
            if let Some(vid) = result { f.instruction(&WI::LocalSet(vl[&vid])); }
            else { f.instruction(&WI::Drop); }
        }

        let can_throw = matches!(
            op,
            Op::BinOp { .. }
                | Op::UnOp { .. }
                | Op::PropGet { .. }
                | Op::PropSet { .. }
                | Op::ArrayGet { .. }
                | Op::ArraySet { .. }
                | Op::Call { .. }
                | Op::HostCall { .. }
        );
        if can_throw {
            self.check_exception(f, im, *nesting_depth, active_try_stack);
        }
        Ok(())
    }

    fn get_funcref(&self, func_id: FuncId, vid: ValueId) -> Option<FuncId> {
        let func = &self.module.functions[func_id];
        match &func.values[vid].kind {
            v12_ir::ValueKind::Inst { block, index } => {
                let inst = &func.blocks[*block].insts[*index];
                if let Op::FuncRef(fid) = &inst.op {
                    Some(*fid)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn const_val(&self, func_id: FuncId, vid: ValueId) -> Option<Const> {
        let func = &self.module.functions[func_id];
        match &func.values[vid].kind {
            v12_ir::ValueKind::Const(c) => Some(c.clone()),
            v12_ir::ValueKind::Inst { block, index } => {
                let inst = &func.blocks[*block].insts[*index];
                if let Op::Const(c) = &inst.op {
                    Some(c.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn load(&self, f: &mut WasmFunc, vl: &IndexMap<ValueId,u32>, im: &IndexMap<&'static str,u32>, vid: ValueId) {
        if let Some(&l) = vl.get(&vid) { f.instruction(&WI::LocalGet(l)); }
        else { f.instruction(&WI::Call(im["rt_undefined"])); }
    }

    fn load_or_undef(&self, val: Option<ValueId>, f: &mut WasmFunc, vl: &IndexMap<ValueId,u32>, im: &IndexMap<&'static str,u32>) {
        match val { Some(v) => self.load(f, vl, im, v), None => { f.instruction(&WI::Call(im["rt_undefined"])); } }
    }

    fn find_term(blk: &BasicBlock) -> Option<usize> {
        blk.insts.iter().position(|i| matches!(&i.op, Op::Return(_)|Op::ReturnCall{..}|Op::Jump(_)|Op::Branch{..}|Op::Throw(_)))
    }

    fn check_exception(
        &self,
        f: &mut WasmFunc,
        im: &IndexMap<&'static str, u32>,
        nesting_depth: u32,
        active_try_stack: &[u32],
    ) {
        if let Some(&try_depth) = active_try_stack.last() {
            f.instruction(&WI::Call(im["rt_has_exception"]));
            let br_label = nesting_depth - try_depth - 2;
            f.instruction(&WI::BrIf(br_label));
        }
    }

    fn emit_jump(
        &self,
        target: BlockId,
        f: &mut WasmFunc,
        nesting_depth: u32,
        loop_stack: &[(usize, u32, BlockId, BlockId, Option<BlockId>)],
        vl: &IndexMap<ValueId, u32>,
        im: &IndexMap<&'static str, u32>,
        blocks: &[(BlockId, &BasicBlock)],
        func_id: FuncId,
    ) -> Result<()> {
        for (_p, &(_, lh_depth, cond_bb, after_bb, update_bb)) in loop_stack.iter().rev().enumerate() {
            if target == after_bb {
                let br_label = nesting_depth - lh_depth + 1;
                f.instruction(&WI::Br(br_label));
                return Ok(());
            }
            if target == cond_bb {
                let br_label = nesting_depth - lh_depth;
                f.instruction(&WI::Br(br_label));
                return Ok(());
            }
            if let Some(up_bb) = update_bb {
                if target == up_bb {
                    if let Some(up_blk) = blocks.iter().find(|(bid, _)| *bid == up_bb).map(|(_, b)| b) {
                        let term = Self::find_term(up_blk);
                        let end = term.unwrap_or(up_blk.insts.len());
                        let mut dummy_consumed = HashSet::new();
                        let mut dummy_try_stack = Vec::new();
                        let mut dummy_depth = nesting_depth;
                        for inst in &up_blk.insts[..end] {
                            self.emit_inst(
                                &inst.op, inst.result, f, vl, im, &[], 0,
                                &mut dummy_depth, &mut dummy_try_stack, &mut dummy_consumed,
                                &IndexMap::new(), blocks,
                                func_id,
                                loop_stack,
                            )?;
                        }
                    }
                    let br_label = nesting_depth - lh_depth;
                    f.instruction(&WI::Br(br_label));
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn order_blocks_dfs(
        blocks: &Arena<BasicBlock>,
        entry_id: BlockId,
    ) -> Vec<BlockId> {
        let mut visited = HashSet::new();
        let mut order = Vec::new();
        
        fn dfs(
            bid: BlockId,
            blocks: &Arena<BasicBlock>,
            visited: &mut HashSet<BlockId>,
            order: &mut Vec<BlockId>,
        ) {
            if !visited.insert(bid) {
                return;
            }
            if let Some(blk) = blocks.get(bid) {
                let term = blk.insts.iter().position(|i| matches!(&i.op, Op::Return(_)|Op::ReturnCall{..}|Op::Jump(_)|Op::Branch{..}|Op::Throw(_)|Op::TryBegin{..}));
                if let Some(ti) = term {
                    match blk.insts[ti].op.clone() {
                        Op::Jump(t) => {
                            dfs(t, blocks, visited, order);
                        }
                        Op::Branch { then_bb, else_bb, .. } => {
                            dfs(else_bb, blocks, visited, order);
                            dfs(then_bb, blocks, visited, order);
                        }
                        Op::TryBegin { body_bb, catch_bb, merge_bb, .. } => {
                             dfs(merge_bb, blocks, visited, order);
                             dfs(catch_bb, blocks, visited, order);
                             dfs(body_bb, blocks, visited, order);
                         }
                        _ => {}
                    }
                }
            }
            order.push(bid);
        }
        
        dfs(entry_id, blocks, &mut visited, &mut order);
        order.reverse();
        order
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_try_body_blocks(
        &self,
        bid:      BlockId,
        f:        &mut WasmFunc,
        vl:       &IndexMap<ValueId, u32>,
        im:       &IndexMap<&'static str, u32>,
        bidx:     &IndexMap<BlockId, usize>,
        blocks:   &[(BlockId, &BasicBlock)],
        func_ids: &[FuncId],
        n_imp:    u32,
        nesting_depth: &mut u32,
        active_try_stack: &mut Vec<u32>,
        consumed: &mut HashSet<usize>,
        func_id: FuncId,
        loop_stack: &mut Vec<(usize, u32, BlockId, BlockId, Option<BlockId>)>,
        merge_bb: BlockId,
        catch_bb: BlockId,
        loop_cond_idxs: &HashSet<usize>,
    ) -> Result<()> {
        let Some(&idx) = bidx.get(&bid) else { return Ok(()); };
        if consumed.contains(&idx) {
            self.emit_jump(bid, f, *nesting_depth, loop_stack, vl, im, blocks, func_id)?;
            return Ok(());
        }
        consumed.insert(idx);

        let blk = blocks[idx].1;

        let is_loop = loop_cond_idxs.contains(&idx);
        if is_loop {
            f.instruction(&WI::Block(BlockType::Empty));
            f.instruction(&WI::Loop(BlockType::Empty));

            let cond_bb = bid;
            let after_bb = if let Some(bt) = Self::find_term(blk) {
                match &blk.insts[bt].op {
                    Op::Branch { else_bb, .. } => *else_bb,
                    _ => cond_bb,
                }
            } else { cond_bb };

            let mut update_bb = None;
            for (obid, bblk) in blocks {
                let obid = *obid;
                if let Some(ti) = Self::find_term(bblk) {
                    if let Op::Jump(t) = &bblk.insts[ti].op {
                        if *t == cond_bb {
                            update_bb = Some(obid);
                            break;
                        }
                    }
                }
            }

            loop_stack.push((idx, *nesting_depth + 2, cond_bb, after_bb, update_bb));
            *nesting_depth += 2;
        }

        let term = Self::find_term(blk);
        let end  = term.unwrap_or(blk.insts.len());

        for inst in &blk.insts[..end] {
            self.emit_inst(
                &inst.op, inst.result, f, vl, im, func_ids, n_imp,
                nesting_depth, active_try_stack, consumed,
                bidx, blocks, func_id, loop_stack
            )?;
        }

        if let Some(ti) = term {
            match &blk.insts[ti].op {
                Op::Return(val) => {
                    self.load_or_undef(*val, f, vl, im);
                    f.instruction(&WI::Return);
                }
                Op::ReturnCall { callee, this, args } => {
                    let mut static_callee = None;
                    if let Some(callee_fid) = self.get_funcref(func_id, *callee) {
                        static_callee = Some(callee_fid);
                    }
                    if let Some(fid) = static_callee {
                        for a in args {
                            self.load(f, vl, im, *a);
                            f.instruction(&WI::Call(im["rt_push_arg"]));
                            f.instruction(&WI::Drop);
                        }
                        self.load(f, vl, im, *this);
                        f.instruction(&WI::I32Const(args.len() as i32));
                        f.instruction(&WI::Call(im["rt_tail_call_frame"]));
                        let idx = func_ids.iter().position(|x| *x == fid).unwrap() as u32;
                        f.instruction(&WI::ReturnCall(n_imp + idx));
                    } else {
                        for a in args {
                            self.load(f, vl, im, *a);
                            f.instruction(&WI::Call(im["rt_push_arg"]));
                            f.instruction(&WI::Drop);
                        }
                        self.load(f, vl, im, *callee);
                        self.load(f, vl, im, *this);
                        f.instruction(&WI::I32Const(args.len() as i32));
                        f.instruction(&WI::Call(im["rt_call"]));
                        f.instruction(&WI::Return);
                    }
                }
                Op::Throw(val) => {
                    self.load_or_undef(Some(*val), f, vl, im);
                    f.instruction(&WI::Call(im["rt_throw"]));
                    f.instruction(&WI::Drop);
                    if let Some(&try_depth) = active_try_stack.last() {
                        let br_label = *nesting_depth - try_depth - 2;
                        f.instruction(&WI::Br(br_label));
                    } else {
                        f.instruction(&WI::Unreachable);
                    }
                }
                Op::Jump(target) => {
                    let target = *target;
                    if target == merge_bb || target == catch_bb {
                        self.emit_jump(target, f, *nesting_depth, loop_stack, vl, im, blocks, func_id)?;
                    } else {
                        self.emit_try_body_blocks(
                            target, f, vl, im, bidx, blocks, func_ids, n_imp,
                            nesting_depth, active_try_stack, consumed, func_id, loop_stack,
                            merge_bb, catch_bb, loop_cond_idxs
                        )?;
                    }
                }
                Op::Branch { cond, then_bb, else_bb } => {
                    let cond = *cond;
                    let then_bb = *then_bb;
                    let else_bb = *else_bb;

                    self.load_or_undef(Some(cond), f, vl, im);
                    f.instruction(&WI::Call(im["rt_truthy_i32"]));
                    f.instruction(&WI::If(BlockType::Empty));
                    *nesting_depth += 1;

                    // Then arm
                    if then_bb == merge_bb || then_bb == catch_bb {
                        self.emit_jump(then_bb, f, *nesting_depth, loop_stack, vl, im, blocks, func_id)?;
                    } else {
                        self.emit_try_body_blocks(
                            then_bb, f, vl, im, bidx, blocks, func_ids, n_imp,
                            nesting_depth, active_try_stack, consumed, func_id, loop_stack,
                            merge_bb, catch_bb, loop_cond_idxs
                        )?;
                    }

                    f.instruction(&WI::Else);

                    // Else arm
                    if else_bb == merge_bb || else_bb == catch_bb {
                        self.emit_jump(else_bb, f, *nesting_depth, loop_stack, vl, im, blocks, func_id)?;
                    } else {
                        self.emit_try_body_blocks(
                            else_bb, f, vl, im, bidx, blocks, func_ids, n_imp,
                            nesting_depth, active_try_stack, consumed, func_id, loop_stack,
                            merge_bb, catch_bb, loop_cond_idxs
                        )?;
                    }

                    f.instruction(&WI::End);
                    *nesting_depth -= 1;
                }
                _ => {}
            }
        }

        if is_loop {
            f.instruction(&WI::End);
            f.instruction(&WI::End);
            loop_stack.pop();
            *nesting_depth -= 2;
        }

        Ok(())
    }
}

fn binop(op: BinOp) -> &'static str {
    match op {
        BinOp::Add=>"rt_add",BinOp::Sub=>"rt_sub",BinOp::Mul=>"rt_mul",BinOp::Div=>"rt_div",
        BinOp::Rem=>"rt_rem",BinOp::Pow=>"rt_pow",BinOp::BitAnd=>"rt_bit_and",
        BinOp::BitOr=>"rt_bit_or",BinOp::BitXor=>"rt_bit_xor",
        BinOp::Shl=>"rt_shl",BinOp::Shr=>"rt_shr",BinOp::UShr=>"rt_ushr",
        BinOp::Eq=>"rt_eq",BinOp::StrictEq=>"rt_strict_eq",
        BinOp::Ne=>"rt_ne",BinOp::StrictNe=>"rt_strict_ne",
        BinOp::Lt=>"rt_lt",BinOp::Le=>"rt_le",BinOp::Gt=>"rt_gt",BinOp::Ge=>"rt_ge",
        BinOp::And=>"rt_and",BinOp::Or=>"rt_or",BinOp::Nullish=>"rt_nullish",
        BinOp::In=>"rt_in",BinOp::InstanceOf=>"rt_instanceof",
    }
}

fn unop(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg=>"rt_neg",UnOp::Not=>"rt_not",UnOp::BitNot=>"rt_bit_not",
        UnOp::TypeOf=>"rt_typeof",UnOp::Void=>"rt_void",UnOp::Delete=>"rt_delete",
        UnOp::PreInc=>"rt_pre_inc",UnOp::PreDec=>"rt_pre_dec",
        UnOp::PostInc=>"rt_post_inc",UnOp::PostDec=>"rt_post_dec",
    }
}

fn hcall(name: &str) -> &'static str {
    match name {
        "console_log"=>"rt_console_log","console_error"=>"rt_console_error",
        "console_warn"=>"rt_console_warn","eval_js"=>"rt_eval_js",
        "global_get"=>"rt_global_get","global_set"=>"rt_global_set",
        "js_await"=>"rt_js_await","js_new"=>"rt_js_new",
        "rt_get_this"=>"rt_get_this","rt_get_exception"=>"rt_get_exception",
        "rt_object_spread"=>"rt_object_spread","rt_array_spread"=>"rt_array_spread",
        "rt_array_push"=>"rt_array_push","rt_get_keys"=>"rt_get_keys",
        "rt_get_values"=>"rt_get_values",
        _=>"rt_undefined",
    }
}
