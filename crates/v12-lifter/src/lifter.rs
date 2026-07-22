//! Lifter: walks the Oxc AST and emits V12 SSA IR.
//!
//! Supports (Phase 1-6):
//!   - Literals (number, string, bool, null, undefined)
//!   - Variable declarations (let, const, var)
//!   - Assignment expressions
//!   - Binary/unary expressions
//!   - If/else statements
//!   - While loops
//!   - For loops (basic C-style)
//!   - Function declarations and function expressions
//!   - Call expressions (including console.log)
//!   - Return statements
//!   - Block statements
//!   - Object literals
//!   - Array literals
//!   - Member expressions (property access)
//!   - Template literals (basic)
//!   - throw statements
//!   - try/catch/finally

use anyhow::{bail, Result};
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, BindingPatternKind, BindingPattern,
    Expression, ForStatementInit, ObjectPropertyKind, Program, PropertyKey,
    SimpleAssignmentTarget, Statement, TemplateLiteral,
};
use v12_ir::{
    BinOp, FuncId, Function, IrBuilder, IrModule, JsType, UnOp, ValueId,
};

use crate::scope::Scope;

pub struct Lifter {
    pub module: IrModule,
}

impl Lifter {
    pub fn new() -> Self {
        Self { module: IrModule::new() }
    }

    /// Entry point: lift an entire program.
    pub fn lift_program(mut self, program: &Program<'_>) -> Result<IrModule> {
        // Allocate the top-level "main" function.
        let main_id = self.module.functions.alloc(Function::new("__main__", vec![]));
        self.module.main = Some(main_id);

        // Lift all top-level statements.
        let mut ctx = LiftCtx::new(&mut self.module, main_id);
        for stmt in &program.body {
            ctx.lift_stmt(stmt)?;
        }

        // Ensure the function ends with a return.
        let undef = ctx.builder.const_undefined();
        ctx.builder.return_(Some(undef));

        Ok(self.module)
    }
}

struct LoopFrame {
    label: Option<String>,
    cond_bb: v12_ir::BlockId,
    after_bb: v12_ir::BlockId,
}

// ─── Lift Context ────────────────────────────────────────────────────────────

struct LiftCtx<'m> {
    builder: IrBuilder<'m>,
    scope:   Scope,
    loop_stack: Vec<LoopFrame>,
    active_label: Option<String>,
    in_unroll: bool,
    slot_types: std::collections::HashMap<u32, v12_ir::JsType>,
}

impl<'m> LiftCtx<'m> {
    fn new(module: &'m mut IrModule, func_id: FuncId) -> Self {
        Self {
            builder: IrBuilder::new(module, func_id),
            scope:   Scope::new(),
            loop_stack: Vec::new(),
            active_label: None,
            in_unroll: false,
            slot_types: std::collections::HashMap::new(),
        }
    }

    fn local_set(&mut self, slot: u32, val: ValueId) {
        self.builder.local_set(slot, val);
        let ty = self.builder.value_type(val);
        self.slot_types.insert(slot, ty);
    }

    // ─── Statements ──────────────────────────────────────────────────────────

    fn lift_stmt(&mut self, stmt: &Statement<'_>) -> Result<()> {
        match stmt {
            Statement::BlockStatement(block) => {
                self.scope.push();
                for s in &block.body {
                    self.lift_stmt(s)?;
                }
                self.scope.pop();
            }

            Statement::VariableDeclaration(decl) => {
                for d in &decl.declarations {
                    let init_val = if let Some(init) = &d.init {
                        self.lift_expr(init)?
                    } else {
                        self.builder.const_undefined()
                    };
                    self.destructure_binding(&d.id, init_val)?;
                }
            }

            Statement::ExpressionStatement(expr_stmt) => {
                self.lift_expr(&expr_stmt.expression)?;
            }

            Statement::ReturnStatement(ret) => {
                let mut tail_call = None;
                if let Some(oxc_ast::ast::Expression::CallExpression(call)) = &ret.argument {
                    let callee = self.lift_expr(&call.callee)?;
                    let args = self.lift_args(&call.arguments)?;
                    let this = if let oxc_ast::ast::Expression::StaticMemberExpression(mem) = &call.callee {
                        self.lift_expr(&mem.object)?
                    } else if let oxc_ast::ast::Expression::ComputedMemberExpression(mem) = &call.callee {
                        self.lift_expr(&mem.object)?
                    } else {
                        self.builder.const_undefined()
                    };
                    tail_call = Some((callee, this, args));
                }

                if let Some((callee, this, args)) = tail_call {
                    self.builder.return_call(callee, this, args);
                } else {
                    let val = if let Some(arg) = &ret.argument {
                        let v = self.lift_expr(arg)?;
                        Some(v)
                    } else {
                        None
                    };
                    self.builder.return_(val);
                }
            }

            Statement::IfStatement(if_stmt) => {
                self.lift_if(if_stmt)?;
            }

            Statement::WhileStatement(while_stmt) => {
                self.lift_while(while_stmt)?;
            }

            Statement::ForStatement(for_stmt) => {
                self.lift_for(for_stmt)?;
            }

            Statement::ForInStatement(for_in) => {
                let obj_val = self.lift_expr(&for_in.right)?;
                let keys_arr = self.builder.host_call("rt_get_keys", vec![obj_val]);
                let length_key = self.builder.const_string("length");
                let keys_len = self.builder.prop_get(keys_arr, length_key);

                let i_name = format!("__for_in_i_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                let i_slot = self.builder.local_new(&i_name);
                let zero = self.builder.const_int(0);
                self.local_set(i_slot, zero);

                let cond_bb = self.builder.new_block("for_in.cond");
                let body_bb = self.builder.new_block("for_in.body");
                let after_bb = self.builder.new_block("for_in.after");

                let label = self.active_label.take();
                self.loop_stack.push(LoopFrame {
                    cond_bb,
                    after_bb,
                    label,
                });

                self.builder.jump(cond_bb);
                self.builder.switch_to(cond_bb);

                let i_val = self.builder.local_get(i_slot);
                let cond_val = self.builder.binop(BinOp::Lt, i_val, keys_len);
                let cond_bool = self.builder.truthy(cond_val);
                self.builder.branch(cond_bool, body_bb, after_bb);

                self.builder.switch_to(body_bb);
                self.scope.push();

                let key_val = self.builder.prop_get(keys_arr, i_val);

                if let oxc_ast::ast::ForStatementLeft::VariableDeclaration(decl) = &for_in.left {
                    for d in &decl.declarations {
                        self.destructure_binding(&d.id, key_val)?;
                    }
                } else if let Some(at) = for_in.left.as_assignment_target() {
                    self.destructure_assignment(at, key_val)?;
                }

                self.lift_stmt(&for_in.body)?;
                self.scope.pop();

                let i_val2 = self.builder.local_get(i_slot);
                let one = self.builder.const_int(1);
                let next_i = self.builder.binop(BinOp::Add, i_val2, one);
                self.local_set(i_slot, next_i);
                self.builder.jump(cond_bb);

                self.loop_stack.pop();
                self.builder.switch_to(after_bb);
            }

            Statement::ForOfStatement(for_of) => {
                let obj_val = self.lift_expr(&for_of.right)?;
                let vals_arr = self.builder.host_call("rt_get_values", vec![obj_val]);
                let length_key = self.builder.const_string("length");
                let vals_len = self.builder.prop_get(vals_arr, length_key);

                let i_name = format!("__for_of_i_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                let i_slot = self.builder.local_new(&i_name);
                let zero = self.builder.const_int(0);
                self.local_set(i_slot, zero);

                let cond_bb = self.builder.new_block("for_of.cond");
                let body_bb = self.builder.new_block("for_of.body");
                let after_bb = self.builder.new_block("for_of.after");

                let label = self.active_label.take();
                self.loop_stack.push(LoopFrame {
                    cond_bb,
                    after_bb,
                    label,
                });

                self.builder.jump(cond_bb);
                self.builder.switch_to(cond_bb);

                let i_val = self.builder.local_get(i_slot);
                let cond_val = self.builder.binop(BinOp::Lt, i_val, vals_len);
                let cond_bool = self.builder.truthy(cond_val);
                self.builder.branch(cond_bool, body_bb, after_bb);

                self.builder.switch_to(body_bb);
                self.scope.push();

                let val_val = self.builder.prop_get(vals_arr, i_val);

                if let oxc_ast::ast::ForStatementLeft::VariableDeclaration(decl) = &for_of.left {
                    for d in &decl.declarations {
                        self.destructure_binding(&d.id, val_val)?;
                    }
                } else if let Some(at) = for_of.left.as_assignment_target() {
                    self.destructure_assignment(at, val_val)?;
                }

                self.lift_stmt(&for_of.body)?;
                self.scope.pop();

                let i_val2 = self.builder.local_get(i_slot);
                let one = self.builder.const_int(1);
                let next_i = self.builder.binop(BinOp::Add, i_val2, one);
                self.local_set(i_slot, next_i);
                self.builder.jump(cond_bb);

                self.loop_stack.pop();
                self.builder.switch_to(after_bb);
            }

            Statement::FunctionDeclaration(func) => {
                let func_id = lift_function_body(
                    self.builder.module,
                    func.id.as_ref().map(|id| id.name.as_str()),
                    &func.params,
                    func.body.as_ref().map(|b| b.as_ref()),
                )?;
                // Bind the function name
                if let Some(id) = &func.id {
                    let name = id.name.as_str().to_string();
                    let func_ref = self.builder.func_ref(func_id);
                    if self.scope.depth() == 1 {
                        let key = self.builder.const_string(&name);
                        self.builder.host_call("global_set", vec![key, func_ref]);
                    } else {
                        let slot = self.builder.local_new(name.clone());
                        self.scope.define(name, slot);
                        self.local_set(slot, func_ref);
                    }
                }
            }

            Statement::ThrowStatement(throw) => {
                let val = self.lift_expr(&throw.argument)?;
                self.builder.throw(val);
            }

            Statement::TryStatement(try_stmt) => {
                self.lift_try(try_stmt)?;
            }

            Statement::BreakStatement(brk) => {
                let target_label = brk.label.as_ref().map(|l| l.name.as_str());
                if let Some(target) = target_label {
                    if let Some(frame) = self.loop_stack.iter().rev().find(|f| f.label.as_deref() == Some(target)) {
                        self.builder.jump(frame.after_bb);
                    } else {
                        bail!("Break label not found: {}", target);
                    }
                } else {
                    if let Some(frame) = self.loop_stack.last() {
                        self.builder.jump(frame.after_bb);
                    } else {
                        bail!("Break outside of loop");
                    }
                }
                let dummy = self.builder.new_block("break.dummy");
                self.builder.switch_to(dummy);
            }

            Statement::ContinueStatement(cont) => {
                let target_label = cont.label.as_ref().map(|l| l.name.as_str());
                if let Some(target) = target_label {
                    if let Some(frame) = self.loop_stack.iter().rev().find(|f| f.label.as_deref() == Some(target)) {
                        self.builder.jump(frame.cond_bb);
                    } else {
                        bail!("Continue label not found: {}", target);
                    }
                } else {
                    if let Some(frame) = self.loop_stack.last() {
                        self.builder.jump(frame.cond_bb);
                    } else {
                        bail!("Continue outside of loop");
                    }
                }
                let dummy = self.builder.new_block("continue.dummy");
                self.builder.switch_to(dummy);
            }

            Statement::LabeledStatement(labeled) => {
                let label_name = labeled.label.name.as_str().to_string();
                self.active_label = Some(label_name);
                self.lift_stmt(&labeled.body)?;
            }

            Statement::ClassDeclaration(cls) => {
                self.lift_class(cls)?;
            }

            Statement::EmptyStatement(_) => {}

            _ => {
                log::warn!("unimplemented statement: {:?}", std::mem::discriminant(stmt));
            }
        }
        Ok(())
    }

    // ─── If ──────────────────────────────────────────────────────────────────

    fn lift_if(&mut self, if_stmt: &oxc_ast::ast::IfStatement<'_>) -> Result<()> {
        let cond_val = self.lift_expr(&if_stmt.test)?;
        let cond_bool = self.builder.truthy(cond_val);

        let then_bb   = self.builder.new_block("if.then");
        let else_bb   = self.builder.new_block("if.else");
        let merge_bb  = self.builder.new_block("if.merge");

        self.builder.branch(cond_bool, then_bb, else_bb);

        // then
        self.builder.switch_to(then_bb);
        self.lift_stmt(&if_stmt.consequent)?;
        self.builder.jump(merge_bb);

        // else
        self.builder.switch_to(else_bb);
        if let Some(alt) = &if_stmt.alternate {
            self.lift_stmt(alt)?;
        }
        self.builder.jump(merge_bb);

        self.builder.switch_to(merge_bb);
        Ok(())
    }

    // ─── While ───────────────────────────────────────────────────────────────

    fn lift_while(&mut self, stmt: &oxc_ast::ast::WhileStatement<'_>) -> Result<()> {
        let cond_bb  = self.builder.new_block("while.cond");
        let body_bb  = self.builder.new_block("while.body");
        let after_bb = self.builder.new_block("while.after");

        let label = self.active_label.take();
        self.loop_stack.push(LoopFrame {
            label,
            cond_bb,
            after_bb,
        });

        self.builder.jump(cond_bb);

        // condition
        self.builder.switch_to(cond_bb);
        let cond_val  = self.lift_expr(&stmt.test)?;
        let cond_bool = self.builder.truthy(cond_val);
        self.builder.branch(cond_bool, body_bb, after_bb);

        // body
        self.builder.switch_to(body_bb);
        self.lift_stmt(&stmt.body)?;
        self.builder.jump(cond_bb);

        self.loop_stack.pop();

        self.builder.switch_to(after_bb);
        Ok(())
    }

    // ─── For ─────────────────────────────────────────────────────────────────

    fn detect_static_for(&self, stmt: &oxc_ast::ast::ForStatement<'_>) -> Option<(String, i32, i32)> {
        let init = stmt.init.as_ref()?;
        let var_name;
        let init_val;
        match init {
            ForStatementInit::VariableDeclaration(decl) if decl.declarations.len() == 1 => {
                let d = &decl.declarations[0];
                if let BindingPatternKind::BindingIdentifier(id) = &d.id.kind {
                    var_name = id.name.to_string();
                    if let Some(Expression::NumericLiteral(num)) = &d.init {
                        init_val = num.value as i32;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }

        let test = stmt.test.as_ref()?;
        let limit_val;
        match test {
            Expression::BinaryExpression(bin) => {
                if let Expression::Identifier(id) = &bin.left {
                    if id.name.as_str() == var_name {
                        if let Expression::NumericLiteral(num) = &bin.right {
                            let op = bin.operator;
                            if op == oxc_ast::ast::BinaryOperator::LessThan {
                                limit_val = num.value as i32;
                            } else if op == oxc_ast::ast::BinaryOperator::LessEqualThan {
                                limit_val = (num.value as i32) + 1;
                            } else {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }

        let update = stmt.update.as_ref()?;
        match update {
            Expression::UpdateExpression(up) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &up.argument {
                    if id.name.as_str() == var_name {
                        // i++ or ++i
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            Expression::AssignmentExpression(assign) => {
                if let Some(SimpleAssignmentTarget::AssignmentTargetIdentifier(id)) = assign.left.as_simple_assignment_target() {
                    if id.name.as_str() == var_name {
                        if assign.operator == oxc_ast::ast::AssignmentOperator::Addition {
                            if let Expression::NumericLiteral(num) = &assign.right {
                                if num.value == 1.0 {
                                    // i += 1
                                } else {
                                    return None;
                                }
                            } else {
                                return None;
                            }
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }

        if limit_val > init_val {
            Some((var_name, init_val, limit_val))
        } else {
            None
        }
    }

    fn check_no_mutation(&self, stmt: &oxc_ast::ast::Statement<'_>, var_name: &str) -> bool {
        match stmt {
            oxc_ast::ast::Statement::ExpressionStatement(expr) => {
                self.check_expr_no_mutation(&expr.expression, var_name)
            }
            oxc_ast::ast::Statement::BlockStatement(blk) => {
                blk.body.iter().all(|s| self.check_no_mutation(s, var_name))
            }
            oxc_ast::ast::Statement::IfStatement(ifs) => {
                self.check_expr_no_mutation(&ifs.test, var_name) &&
                self.check_no_mutation(&ifs.consequent, var_name) &&
                ifs.alternate.as_ref().map(|s| self.check_no_mutation(s, var_name)).unwrap_or(true)
            }
            _ => true,
        }
    }

    fn check_expr_no_mutation(&self, expr: &oxc_ast::ast::Expression<'_>, var_name: &str) -> bool {
        match expr {
            oxc_ast::ast::Expression::AssignmentExpression(assign) => {
                if let Some(SimpleAssignmentTarget::AssignmentTargetIdentifier(id)) = assign.left.as_simple_assignment_target() {
                    if id.name.as_str() == var_name {
                        return false;
                    }
                }
                self.check_expr_no_mutation(&assign.right, var_name)
            }
            oxc_ast::ast::Expression::UpdateExpression(up) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &up.argument {
                    if id.name.as_str() == var_name {
                        return false;
                    }
                }
                true
            }
            oxc_ast::ast::Expression::BinaryExpression(bin) => {
                self.check_expr_no_mutation(&bin.left, var_name) && self.check_expr_no_mutation(&bin.right, var_name)
            }
            oxc_ast::ast::Expression::CallExpression(call) => {
                self.check_expr_no_mutation(&call.callee, var_name) &&
                call.arguments.iter().all(|arg| {
                    if let Some(e) = arg.as_expression() {
                        self.check_expr_no_mutation(e, var_name)
                    } else {
                        true
                    }
                })
            }
            _ => true,
        }
    }

    fn lift_for(&mut self, stmt: &oxc_ast::ast::ForStatement<'_>) -> Result<()> {
        if let Some((var_name, init_val, limit_val)) = self.detect_static_for(stmt) {
            if self.check_no_mutation(&stmt.body, &var_name) {
                if self.try_vectorize_loop(stmt, &var_name, init_val, limit_val)? {
                    return Ok(());
                }
                if !self.in_unroll && (limit_val - init_val) <= 32 {
                    let slot = self.builder.local_new(&var_name);
                    self.scope.define(var_name.clone(), slot);
                    self.in_unroll = true;
                    for val in init_val..limit_val {
                        let v = self.builder.const_int(val);
                        self.local_set(slot, v);
                        self.lift_stmt(&stmt.body)?;
                    }
                    self.in_unroll = false;
                    return Ok(());
                }
            }
        }
        // init
        if let Some(init) = &stmt.init {
            match init {
                ForStatementInit::VariableDeclaration(decl) => {
                    for d in &decl.declarations {
                        let init_val = if let Some(init_expr) = &d.init {
                            self.lift_expr(init_expr)?
                        } else {
                            self.builder.const_undefined()
                        };
                        self.destructure_binding(&d.id, init_val)?;
                    }
                }
                ForStatementInit::BooleanLiteral(e) => {
                    self.builder.const_bool(e.value);
                }
                ForStatementInit::NullLiteral(_) => {}
                ForStatementInit::NumericLiteral(n) => {
                    self.builder.const_float(n.value);
                }
                ForStatementInit::StringLiteral(s) => {
                    self.builder.const_string(s.value.as_str());
                }
                init => {
                    if let Some(expr) = init.as_expression() {
                        self.lift_expr(expr)?;
                    }
                }
            }
        }

        let cond_bb  = self.builder.new_block("for.cond");
        let body_bb  = self.builder.new_block("for.body");
        let update_bb = self.builder.new_block("for.update");
        let after_bb  = self.builder.new_block("for.after");

        let label = self.active_label.take();
        self.loop_stack.push(LoopFrame {
            label,
            cond_bb: update_bb,
            after_bb,
        });

        self.builder.jump(cond_bb);

        // condition
        self.builder.switch_to(cond_bb);
        if let Some(test) = &stmt.test {
            let cond_val  = self.lift_expr(test)?;
            let cond_bool = self.builder.truthy(cond_val);
            self.builder.branch(cond_bool, body_bb, after_bb);
        } else {
            self.builder.jump(body_bb);
        }

        // body
        self.builder.switch_to(body_bb);
        self.lift_stmt(&stmt.body)?;
        self.builder.jump(update_bb);

        // update
        self.builder.switch_to(update_bb);
        if let Some(update) = &stmt.update {
            self.lift_expr(update)?;
        }
        self.builder.jump(cond_bb);

        self.loop_stack.pop();

        self.builder.switch_to(after_bb);
        Ok(())
    }

    // ─── Try / Catch / Finally ───────────────────────────────────────────────

    fn lift_try(&mut self, stmt: &oxc_ast::ast::TryStatement<'_>) -> Result<()> {
        let try_bb = self.builder.new_block("try_body");
        let catch_bb = self.builder.new_block("try_catch");
        let merge_bb = self.builder.new_block("try_merge");

        self.builder.try_begin(try_bb, catch_bb, None, merge_bb);

        // Try body
        self.builder.switch_to(try_bb);
        self.scope.push();
        for s in &stmt.block.body {
            self.lift_stmt(s)?;
        }
        self.scope.pop();
        self.builder.jump(merge_bb);

        // Catch block
        self.builder.switch_to(catch_bb);
        if let Some(handler) = &stmt.handler {
            self.scope.push();
            if let Some(param) = &handler.param {
                let name = match &param.pattern.kind {
                    BindingPatternKind::BindingIdentifier(id) => id.name.as_str().to_string(),
                    _ => "__catch_err__".to_string(),
                };
                let slot = self.builder.local_new(name.clone());
                self.scope.define(name, slot);
                
                let exc_val = self.builder.host_call("rt_get_exception".to_string(), vec![]);
                self.local_set(slot, exc_val);
            }
            for s in &handler.body.body {
                self.lift_stmt(s)?;
            }
            self.scope.pop();
        }
        self.builder.jump(merge_bb);

        self.builder.switch_to(merge_bb);
        Ok(())
    }

    // ─── Expressions ─────────────────────────────────────────────────────────

    fn lift_expr(&mut self, expr: &Expression<'_>) -> Result<ValueId> {
        match expr {
            Expression::NullLiteral(_) => Ok(self.builder.const_null()),
            Expression::BooleanLiteral(b) => Ok(self.builder.const_bool(b.value)),

            Expression::NumericLiteral(n) => {
                let v = n.value;
                if v.fract() == 0.0 && v >= i32::MIN as f64 && v <= i32::MAX as f64 {
                    Ok(self.builder.const_int(v as i32))
                } else {
                    Ok(self.builder.const_float(v))
                }
            }

            Expression::StringLiteral(s) => {
                Ok(self.builder.const_string(s.value.as_str()))
            }

            Expression::TemplateLiteral(tpl) => self.lift_template(tpl),

            Expression::Identifier(id) => {
                let name = id.name.as_str();
                if name == "undefined" {
                    return Ok(self.builder.const_undefined());
                }
                match self.scope.lookup(name) {
                    Some(slot) => {
                        let res = self.builder.local_get(slot);
                        if let Some(&ty) = self.slot_types.get(&slot) {
                            let func_id = self.builder.func_id;
                            if let Some(val) = self.builder.module.functions[func_id].values.get_mut(res) {
                                val.ty = ty;
                            }
                        }
                        Ok(res)
                    }
                    None => {
                        // Global — emit as a host lookup.
                        let key = self.builder.const_string(name);
                        Ok(self.builder.host_call("global_get", vec![key]))
                    }
                }
            }

            Expression::AssignmentExpression(assign) => {
                let val = self.lift_expr(&assign.right)?;
                self.destructure_assignment(&assign.left, val)?;
                Ok(val)
            }

            Expression::BinaryExpression(bin) => {
                let lhs = self.lift_expr(&bin.left)?;
                let rhs = self.lift_expr(&bin.right)?;
                let op  = map_binop(bin.operator)?;
                Ok(self.builder.binop(op, lhs, rhs))
            }

            Expression::LogicalExpression(log) => {
                let lhs = self.lift_expr(&log.left)?;
                let rhs = self.lift_expr(&log.right)?;
                let op  = match log.operator {
                    oxc_ast::ast::LogicalOperator::And     => BinOp::And,
                    oxc_ast::ast::LogicalOperator::Or      => BinOp::Or,
                    oxc_ast::ast::LogicalOperator::Coalesce => BinOp::Nullish,
                };
                Ok(self.builder.binop(op, lhs, rhs))
            }

            Expression::UnaryExpression(un) => {
                let val = self.lift_expr(&un.argument)?;
                let op  = map_unop(un.operator)?;
                Ok(self.builder.unop(op, val))
            }

            Expression::UpdateExpression(upd) => {
                // upd.argument is already a SimpleAssignmentTarget.
                // Read the current value, apply the update op, write back.
                let (current_val, write_back) = match &upd.argument {
                    SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                        let name = id.name.as_str().to_string();
                        let val = match self.scope.lookup(&name) {
                            Some(slot) => self.builder.local_get(slot),
                            None => {
                                let key = self.builder.const_string(&name);
                                self.builder.host_call("global_get", vec![key])
                            }
                        };
                        (val, Some(name))
                    }
                    _ => {
                        // Complex target (member expression) — emit undefined.
                        (self.builder.const_undefined(), None)
                    }
                };
                let op = if upd.prefix {
                    if upd.operator == oxc_ast::ast::UpdateOperator::Increment { UnOp::PreInc } else { UnOp::PreDec }
                } else {
                    if upd.operator == oxc_ast::ast::UpdateOperator::Increment { UnOp::PostInc } else { UnOp::PostDec }
                };
                let result = self.builder.unop(op, current_val);
                if let Some(name) = write_back {
                    if let Some(slot) = self.scope.lookup(&name) {
                        self.local_set(slot, result);
                    }
                }
                Ok(result)
            }

            Expression::CallExpression(call) => {
                // Special: console.log / console.error / console.warn
                if let Expression::StaticMemberExpression(mem) = &call.callee {
                    if let Expression::Identifier(obj) = &mem.object {
                        if obj.name.as_str() == "console" {
                            let host_fn = format!("console_{}", mem.property.name.as_str());
                            let args = self.lift_args(&call.arguments)?;
                            return Ok(self.builder.host_call(host_fn, args));
                        }
                    }
                }

                // eval() — host call that triggers the micro-module synthesizer.
                if let Expression::Identifier(id) = &call.callee {
                    if id.name.as_str() == "eval" {
                        let args = self.lift_args(&call.arguments)?;
                        return Ok(self.builder.host_call("eval_js", args));
                    }
                }

                // Method call: obj.method(args)
                if let Expression::StaticMemberExpression(mem) = &call.callee {
                    let obj = self.lift_expr(&mem.object)?;
                    let key = self.builder.const_string(mem.property.name.as_str());
                    let method = self.builder.prop_get(obj, key);
                    let args = self.lift_args(&call.arguments)?;
                    return Ok(self.builder.call(method, obj, args));
                }

                // Computed method: obj[expr](args)
                if let Expression::ComputedMemberExpression(mem) = &call.callee {
                    let obj = self.lift_expr(&mem.object)?;
                    let key = self.lift_expr(&mem.expression)?;
                    let method = self.builder.prop_get(obj, key);
                    let args = self.lift_args(&call.arguments)?;
                    return Ok(self.builder.call(method, obj, args));
                }

                // Plain function call: f(args)
                let undef = self.builder.const_undefined();
                let callee = self.lift_expr(&call.callee)?;
                let args = self.lift_args(&call.arguments)?;
                Ok(self.builder.call(callee, undef, args))
            }

            Expression::ObjectExpression(obj) => {
                let mut static_keys = Vec::new();
                let mut static_vals = Vec::new();
                let mut has_computed_or_spread = false;

                for prop in &obj.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            let key_name = match &p.key {
                                PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
                                PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
                                PropertyKey::NumericLiteral(n) => Some(n.value.to_string()),
                                _ => None,
                            };
                            if let Some(name) = key_name {
                                static_keys.push(name);
                                static_vals.push(&p.value);
                            } else {
                                has_computed_or_spread = true;
                                break;
                            }
                        }
                        ObjectPropertyKind::SpreadProperty(_) => {
                            has_computed_or_spread = true;
                            break;
                        }
                    }
                }

                if !has_computed_or_spread && !static_keys.is_empty() {
                    let mut vals = Vec::new();
                    for val_expr in static_vals {
                        vals.push(self.lift_expr(val_expr)?);
                    }
                    let shape_id = self.builder.module.register_shape(static_keys);
                    Ok(self.builder.object_shape_new(shape_id, vals))
                } else {
                    let obj_val = self.builder.object_new();
                    for prop in &obj.properties {
                        match prop {
                            ObjectPropertyKind::ObjectProperty(p) => {
                                let key = match &p.key {
                                    PropertyKey::StaticIdentifier(id) => {
                                        self.builder.const_string(id.name.as_str())
                                    }
                                    PropertyKey::StringLiteral(s) => {
                                        self.builder.const_string(s.value.as_str())
                                    }
                                    PropertyKey::NumericLiteral(n) => {
                                        self.builder.const_string(n.value.to_string())
                                    }
                                    k => {
                                        let computed = self.lift_expr(k.as_expression().unwrap())?;
                                        self.builder.to_string(computed)
                                    }
                                };
                                let val = self.lift_expr(&p.value)?;
                                self.builder.prop_set(obj_val, key, val);
                            }
                            ObjectPropertyKind::SpreadProperty(sp) => {
                                let val = self.lift_expr(&sp.argument)?;
                                self.builder.host_call("rt_object_spread", vec![obj_val, val]);
                            }
                        }
                    }
                    Ok(obj_val)
                }
            }

            Expression::ArrayExpression(arr) => {
                let mut elems = Vec::new();
                for el in &arr.elements {
                    match el {
                        ArrayExpressionElement::SpreadElement(_) => {
                            log::warn!("spread in array literal not yet supported");
                        }
                        ArrayExpressionElement::Elision(_) => {
                            elems.push(self.builder.const_undefined());
                        }
                        e => {
                            if let Some(expr) = e.as_expression() {
                                elems.push(self.lift_expr(expr)?);
                            }
                        }
                    }
                }
                Ok(self.builder.array_new(elems))
            }

            Expression::StaticMemberExpression(mem) => {
                let obj = self.lift_expr(&mem.object)?;
                let prop_name = mem.property.name.as_str();
                if let v12_ir::JsType::ObjectShape(shape_id) = self.builder.value_type(obj) {
                    if let Some(field_idx) = self.builder.module.shapes[shape_id as usize].iter().position(|k| k == prop_name) {
                        return Ok(self.builder.shape_get(obj, shape_id, field_idx as u32));
                    }
                }
                let key = self.builder.const_string(prop_name);
                Ok(self.builder.prop_get(obj, key))
            }

            Expression::ComputedMemberExpression(mem) => {
                let obj = self.lift_expr(&mem.object)?;
                let key = self.lift_expr(&mem.expression)?;
                if let v12_ir::JsType::TypedArray { element_size, is_float } = self.builder.value_type(obj) {
                    return Ok(self.builder.mem_load(obj, key, element_size, is_float));
                }
                Ok(self.builder.prop_get(obj, key))
            }

            Expression::FunctionExpression(func) => {
                let id = lift_function_body(
                    self.builder.module,
                    func.id.as_ref().map(|id| id.name.as_str()),
                    &func.params,
                    func.body.as_ref().map(|b| b.as_ref()),
                )?;
                Ok(self.builder.func_ref(id))
            }

            Expression::ArrowFunctionExpression(arrow) => {
                let id = lift_arrow_body(self.builder.module, arrow)?;
                Ok(self.builder.func_ref(id))
            }

            Expression::ClassExpression(cls) => {
                self.lift_class(cls)
            }

            Expression::ConditionalExpression(cond) => {
                let test_val = self.lift_expr(&cond.test)?;
                let test_bool = self.builder.truthy(test_val);

                let then_bb  = self.builder.new_block("ternary.then");
                let else_bb  = self.builder.new_block("ternary.else");
                let merge_bb = self.builder.new_block("ternary.merge");

                self.builder.branch(test_bool, then_bb, else_bb);

                self.builder.switch_to(then_bb);
                let then_val = self.lift_expr(&cond.consequent)?;
                // Store result into a temp slot.
                let slot = self.builder.local_new("__ternary_tmp__");
                self.local_set(slot, then_val);
                self.builder.jump(merge_bb);

                self.builder.switch_to(else_bb);
                let else_val = self.lift_expr(&cond.alternate)?;
                self.local_set(slot, else_val);
                self.builder.jump(merge_bb);

                self.builder.switch_to(merge_bb);
                Ok(self.builder.local_get(slot))
            }

            Expression::NewExpression(new_expr) => {
                let mut is_typed_array = None;
                if let Expression::Identifier(id) = &new_expr.callee {
                    let name = id.name.as_str();
                    match name {
                        "Int8Array" | "Uint8Array" | "Uint8ClampedArray" => {
                            is_typed_array = Some((1, false));
                        }
                        "Int16Array" | "Uint16Array" => {
                            is_typed_array = Some((2, false));
                        }
                        "Int32Array" | "Uint32Array" => {
                            is_typed_array = Some((4, false));
                        }
                        "Float32Array" => {
                            is_typed_array = Some((4, true));
                        }
                        "Float64Array" => {
                            is_typed_array = Some((8, true));
                        }
                        "BigInt64Array" | "BigUint64Array" => {
                            is_typed_array = Some((8, false));
                        }
                        _ => {}
                    }
                }
                let callee = self.lift_expr(&new_expr.callee)?;
                let args   = self.lift_args(&new_expr.arguments)?;
                let mut host_args = vec![callee];
                host_args.extend(args);
                let res = self.builder.host_call("js_new", host_args);
                if let Some((size, is_float)) = is_typed_array {
                    let func_id = self.builder.func_id;
                    if let Some(val) = self.builder.module.functions[func_id].values.get_mut(res) {
                        val.ty = v12_ir::JsType::TypedArray { element_size: size, is_float };
                    }
                }
                Ok(res)
            }

            Expression::SequenceExpression(seq) => {
                let mut last = self.builder.const_undefined();
                for e in &seq.expressions {
                    last = self.lift_expr(e)?;
                }
                Ok(last)
            }

            Expression::YieldExpression(_) => {
                bail!("generators not yet supported")
            }

            Expression::AwaitExpression(aw) => {
                let val = self.lift_expr(&aw.argument)?;
                Ok(self.builder.host_call("js_await", vec![val]))
            }

            Expression::ThisExpression(_) => {
                Ok(self.builder.host_call("rt_get_this".to_string(), vec![]))
            }

            Expression::TaggedTemplateExpression(_) => {
                bail!("tagged template literals not yet supported")
            }

            _ => {
                log::warn!("unimplemented expression");
                Ok(self.builder.const_undefined())
            }
        }
    }

    // ─── Template Literals ───────────────────────────────────────────────────

    fn lift_template(&mut self, tpl: &TemplateLiteral<'_>) -> Result<ValueId> {
        // Interleave quasis (raw strings) and expressions, concatenating with +.
        let mut parts: Vec<ValueId> = Vec::new();

        let quasis = &tpl.quasis;
        let exprs  = &tpl.expressions;

        for (i, quasi) in quasis.iter().enumerate() {
            let s = quasi.value.cooked.as_deref().unwrap_or("");
            if !s.is_empty() || i == 0 {
                parts.push(self.builder.const_string(s));
            }
            if i < exprs.len() {
                let e   = self.lift_expr(&exprs[i])?;
                let str = self.builder.to_string(e);
                parts.push(str);
            }
        }

        // Fold with Add (string concatenation).
        if parts.is_empty() {
            return Ok(self.builder.const_string(""));
        }
        let mut acc = parts.remove(0);
        for p in parts {
            acc = self.builder.binop(BinOp::Add, acc, p);
        }
        Ok(acc)
    }

    // ─── Argument lifting ────────────────────────────────────────────────────

    fn lift_args(&mut self, arguments: &[Argument<'_>]) -> Result<Vec<ValueId>> {
        let mut vals = Vec::new();
        for arg in arguments {
            match arg {
                Argument::SpreadElement(_) => {
                    log::warn!("spread argument not yet supported");
                }
                e => {
                    if let Some(expr) = e.as_expression() {
                        vals.push(self.lift_expr(expr)?);
                    }
                }
            }
        }
        Ok(vals)
    }

    fn lift_class(&mut self, cls: &oxc_ast::ast::Class<'_>) -> Result<ValueId> {
        let class_name = cls.id.as_ref().map(|i| i.name.as_str().to_string()).unwrap_or_else(|| "AnonymousClass".to_string());
        
        let ctor_obj = self.builder.object_new();
        let proto_obj = self.builder.object_new();
        let proto_key = self.builder.const_string("prototype");
        self.builder.prop_set(ctor_obj, proto_key, proto_obj);
        
        for element in &cls.body.body {
            if let oxc_ast::ast::ClassElement::MethodDefinition(method) = element {
                let key_name = match &method.key {
                    oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
                    oxc_ast::ast::PropertyKey::StringLiteral(s) => s.value.as_str(),
                    _ => "method",
                };
                if let Ok(method_id) = lift_function_body(
                    self.builder.module,
                    Some(key_name),
                    &method.value.params,
                    method.value.body.as_ref().map(|b| b.as_ref()),
                ) {
                    let method_ref = self.builder.func_ref(method_id);
                    let key_val = self.builder.const_string(key_name);
                    
                    if method.r#static {
                        self.builder.prop_set(ctor_obj, key_val, method_ref);
                    } else {
                        self.builder.prop_set(proto_obj, key_val, method_ref);
                    }
                }
            }
        }

        if self.scope.depth() == 1 {
            let name_val = self.builder.const_string(&class_name);
            self.builder.host_call("global_set", vec![name_val, ctor_obj]);
        } else if let Some(slot) = self.scope.lookup(&class_name) {
            self.local_set(slot, ctor_obj);
        } else {
            let slot = self.builder.local_new(class_name.clone());
            self.scope.define(class_name, slot);
            self.local_set(slot, ctor_obj);
        }
        
        Ok(ctor_obj)
    }

    fn destructure_binding(&mut self, pattern: &BindingPattern<'_>, source_val: ValueId) -> Result<()> {
        match &pattern.kind {
            BindingPatternKind::BindingIdentifier(id) => {
                let name = id.name.as_str().to_string();
                if self.scope.depth() == 1 {
                    let key = self.builder.const_string(&name);
                    self.builder.host_call("global_set", vec![key, source_val]);
                } else {
                    let slot = match self.scope.lookup(&name) {
                        Some(slot) => slot,
                        None => {
                            let slot = self.builder.local_new(name.clone());
                            self.scope.define(name, slot);
                            slot
                        }
                    };
                    self.local_set(slot, source_val);
                }
            }
            BindingPatternKind::ArrayPattern(arr) => {
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(elem_pat) = elem {
                        let key = self.builder.const_string(&i.to_string());
                        let val = self.builder.prop_get(source_val, key);
                        self.destructure_binding(elem_pat, val)?;
                    }
                }
            }
            BindingPatternKind::ObjectPattern(obj) => {
                for prop in &obj.properties {
                    let key = match &prop.key {
                        PropertyKey::StaticIdentifier(id) => {
                            self.builder.const_string(id.name.as_str())
                        }
                        PropertyKey::StringLiteral(s) => {
                            self.builder.const_string(s.value.as_str())
                        }
                        PropertyKey::NumericLiteral(n) => {
                            self.builder.const_string(n.value.to_string())
                        }
                        k => {
                            let computed = self.lift_expr(k.as_expression().unwrap())?;
                            self.builder.to_string(computed)
                        }
                    };
                    let val = self.builder.prop_get(source_val, key);
                    self.destructure_binding(&prop.value, val)?;
                }
            }
            BindingPatternKind::AssignmentPattern(assign) => {
                let undef = self.builder.const_undefined();
                let is_undef = self.builder.host_call("rt_strict_eq", vec![
                    source_val,
                    undef,
                ]);
                let body_bb = self.builder.new_block("destruct.default");
                let else_bb = self.builder.new_block("destruct.source");
                let merge_bb = self.builder.new_block("destruct.merge");

                let temp_name = format!("__temp_destruct_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                let temp_slot = self.builder.local_new(&temp_name);

                self.builder.branch(is_undef, body_bb, else_bb);

                // Body block (default)
                self.builder.switch_to(body_bb);
                let default_val = self.lift_expr(&assign.right)?;
                self.local_set(temp_slot, default_val);
                self.builder.jump(merge_bb);

                // Else block
                self.builder.switch_to(else_bb);
                self.local_set(temp_slot, source_val);
                self.builder.jump(merge_bb);

                // Merge block
                self.builder.switch_to(merge_bb);
                let final_val = self.builder.local_get(temp_slot);

                self.destructure_binding(&assign.left, final_val)?;
            }
        }
        Ok(())
    }

    fn destructure_assignment(&mut self, at: &oxc_ast::ast::AssignmentTarget<'_>, val: ValueId) -> Result<()> {
        if let Some(simple) = at.as_simple_assignment_target() {
            match simple {
                SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                    let name = id.name.as_str().to_string();
                    match self.scope.lookup(&name) {
                        Some(slot) => self.local_set(slot, val),
                        None => {
                            let key = self.builder.const_string(&name);
                            self.builder.host_call("global_set", vec![key, val]);
                        }
                    }
                }
                SimpleAssignmentTarget::StaticMemberExpression(mem) => {
                    let obj = self.lift_expr(&mem.object)?;
                    let prop_name = mem.property.name.as_str();
                    if let v12_ir::JsType::ObjectShape(shape_id) = self.builder.value_type(obj) {
                        if let Some(field_idx) = self.builder.module.shapes[shape_id as usize].iter().position(|k| k == prop_name) {
                            self.builder.shape_set(obj, shape_id, field_idx as u32, val);
                            return Ok(());
                        }
                    }
                    let key = self.builder.const_string(prop_name);
                    self.builder.prop_set(obj, key, val);
                }
                SimpleAssignmentTarget::ComputedMemberExpression(mem) => {
                    let obj = self.lift_expr(&mem.object)?;
                    let key = self.lift_expr(&mem.expression)?;
                    if let v12_ir::JsType::TypedArray { element_size, is_float } = self.builder.value_type(obj) {
                        self.builder.mem_store(obj, key, element_size, val, is_float);
                        return Ok(());
                    }
                    self.builder.prop_set(obj, key, val);
                }
                _ => {}
            }
        } else if let Some(pat) = at.as_assignment_target_pattern() {
            self.destructure_assignment_pattern(pat, val)?;
        }
        Ok(())
    }

    fn destructure_assignment_pattern(&mut self, pattern: &oxc_ast::ast::AssignmentTargetPattern<'_>, source_val: ValueId) -> Result<()> {
        match pattern {
            oxc_ast::ast::AssignmentTargetPattern::ArrayAssignmentTarget(arr) => {
                for (i, elem) in arr.elements.iter().enumerate() {
                    if let Some(e) = elem {
                        let key = self.builder.const_string(&i.to_string());
                        let val = self.builder.prop_get(source_val, key);
                        
                        if let Some(at) = e.as_assignment_target() {
                            self.destructure_assignment(at, val)?;
                        } else if let oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(atd) = e {
                            let undef = self.builder.const_undefined();
                            let is_undef = self.builder.host_call("rt_strict_eq", vec![
                                val,
                                undef,
                            ]);
                            let temp_name = format!("__temp_destruct_assign_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                            let temp_slot = self.builder.local_new(&temp_name);

                            let body_bb = self.builder.new_block("destruct.default");
                            let else_bb = self.builder.new_block("destruct.source");
                            let merge_bb = self.builder.new_block("destruct.merge");

                            self.builder.branch(is_undef, body_bb, else_bb);

                            self.builder.switch_to(body_bb);
                            let default_val = self.lift_expr(&atd.init)?;
                            self.local_set(temp_slot, default_val);
                            self.builder.jump(merge_bb);

                            self.builder.switch_to(else_bb);
                            self.local_set(temp_slot, val);
                            self.builder.jump(merge_bb);

                            self.builder.switch_to(merge_bb);
                            let final_val = self.builder.local_get(temp_slot);
                            
                            self.destructure_assignment(&atd.binding, final_val)?;
                        }
                    }
                }
            }
            oxc_ast::ast::AssignmentTargetPattern::ObjectAssignmentTarget(obj) => {
                for prop in &obj.properties {
                    match prop {
                        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(atpi) => {
                            let name = atpi.binding.name.as_str().to_string();
                            let key = self.builder.const_string(&name);
                            let val = self.builder.prop_get(source_val, key);
                            
                            let final_val = if let Some(init) = &atpi.init {
                                let undef = self.builder.const_undefined();
                                let is_undef = self.builder.host_call("rt_strict_eq", vec![
                                    val,
                                    undef,
                                ]);
                                let temp_name = format!("__temp_destruct_assign_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                                let temp_slot = self.builder.local_new(&temp_name);

                                let body_bb = self.builder.new_block("destruct.default");
                                let else_bb = self.builder.new_block("destruct.source");
                                let merge_bb = self.builder.new_block("destruct.merge");

                                self.builder.branch(is_undef, body_bb, else_bb);

                                self.builder.switch_to(body_bb);
                                let default_val = self.lift_expr(init)?;
                                self.local_set(temp_slot, default_val);
                                self.builder.jump(merge_bb);

                                self.builder.switch_to(else_bb);
                                self.local_set(temp_slot, val);
                                self.builder.jump(merge_bb);

                                self.builder.switch_to(merge_bb);
                                self.builder.local_get(temp_slot)
                            } else {
                                val
                            };

                            match self.scope.lookup(&name) {
                                Some(slot) => self.local_set(slot, final_val),
                                None => {
                                    let key = self.builder.const_string(&name);
                                    self.builder.host_call("global_set", vec![key, final_val]);
                                }
                            }
                        }
                        oxc_ast::ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(atpp) => {
                            let key = match &atpp.name {
                                PropertyKey::StaticIdentifier(id) => {
                                    self.builder.const_string(id.name.as_str())
                                }
                                PropertyKey::StringLiteral(s) => {
                                    self.builder.const_string(s.value.as_str())
                                }
                                PropertyKey::NumericLiteral(n) => {
                                    self.builder.const_string(n.value.to_string())
                                }
                                k => {
                                    let computed = self.lift_expr(k.as_expression().unwrap())?;
                                    self.builder.to_string(computed)
                                }
                            };
                            let val = self.builder.prop_get(source_val, key);
                            
                            if let Some(at) = atpp.binding.as_assignment_target() {
                                self.destructure_assignment(at, val)?;
                            } else if let oxc_ast::ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(atd) = &atpp.binding {
                                let undef = self.builder.const_undefined();
                                let is_undef = self.builder.host_call("rt_strict_eq", vec![
                                    val,
                                    undef,
                                ]);
                                let temp_name = format!("__temp_destruct_assign_{}__", self.builder.module.functions[self.builder.func_id].local_count);
                                let temp_slot = self.builder.local_new(&temp_name);

                                let body_bb = self.builder.new_block("destruct.default");
                                let else_bb = self.builder.new_block("destruct.source");
                                let merge_bb = self.builder.new_block("destruct.merge");

                                self.builder.branch(is_undef, body_bb, else_bb);

                                self.builder.switch_to(body_bb);
                                let default_val = self.lift_expr(&atd.init)?;
                                self.local_set(temp_slot, default_val);
                                self.builder.jump(merge_bb);

                                self.builder.switch_to(else_bb);
                                self.local_set(temp_slot, val);
                                self.builder.jump(merge_bb);

                                self.builder.switch_to(merge_bb);
                                let final_val = self.builder.local_get(temp_slot);
                                
                                self.destructure_assignment(&atd.binding, final_val)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn try_vectorize_loop(&mut self, stmt: &oxc_ast::ast::ForStatement<'_>, var_name: &str, init_val: i32, limit_val: i32) -> Result<bool> {
        let expr = match &stmt.body {
            Statement::ExpressionStatement(estmt) => Some(&estmt.expression),
            Statement::BlockStatement(blk) if blk.body.len() == 1 => {
                if let Statement::ExpressionStatement(estmt) = &blk.body[0] {
                    Some(&estmt.expression)
                } else {
                    None
                }
            }
            _ => None,
        };
        let Some(Expression::AssignmentExpression(assign)) = expr else {
            return Ok(false);
        };
        let Some(SimpleAssignmentTarget::ComputedMemberExpression(dest_mem)) = assign.left.as_simple_assignment_target() else {
            return Ok(false);
        };
        let Expression::BinaryExpression(bin) = &assign.right else {
            return Ok(false);
        };
        let Expression::ComputedMemberExpression(src1_mem) = &bin.left else {
            return Ok(false);
        };
        let Expression::ComputedMemberExpression(src2_mem) = &bin.right else {
            return Ok(false);
        };

        let Expression::Identifier(dest_idx) = &dest_mem.expression else {
            return Ok(false);
        };
        let Expression::Identifier(src1_idx) = &src1_mem.expression else {
            return Ok(false);
        };
        let Expression::Identifier(src2_idx) = &src2_mem.expression else {
            return Ok(false);
        };
        if dest_idx.name != var_name || src1_idx.name != var_name || src2_idx.name != var_name {
            return Ok(false);
        }

        if bin.operator != oxc_ast::ast::BinaryOperator::Addition {
            return Ok(false);
        }

        let dest_obj = self.lift_expr(&dest_mem.object)?;
        let src1_obj = self.lift_expr(&src1_mem.object)?;
        let src2_obj = self.lift_expr(&src2_mem.object)?;
        let ty_dest = self.builder.value_type(dest_obj);
        let ty_src1 = self.builder.value_type(src1_obj);
        let ty_src2 = self.builder.value_type(src2_obj);

        if let (
            v12_ir::JsType::TypedArray { element_size: size_d, is_float: float_d },
            v12_ir::JsType::TypedArray { element_size: size_s1, is_float: float_s1 },
            v12_ir::JsType::TypedArray { element_size: size_s2, is_float: float_s2 },
        ) = (ty_dest, ty_src1, ty_src2) {
            if size_d == size_s1 && size_s1 == size_s2 && float_d == float_s1 && float_s1 == float_s2 {
                let lane_size = if size_d == 8 { 2 } else { 4 };
                let num_elements = limit_val - init_val;
                if num_elements % lane_size == 0 {
                    for val in (init_val..limit_val).step_by(lane_size as usize) {
                        let offset_val = self.builder.const_int(val);
                        let v1 = self.builder.vec_load128(src1_obj, offset_val);
                        let v2 = self.builder.vec_load128(src2_obj, offset_val);
                        let v_res = if float_d {
                            self.builder.vec_add64x2(v1, v2)
                        } else {
                            self.builder.vec_add32x4(v1, v2)
                        };
                        self.builder.vec_store128(dest_obj, offset_val, v_res);
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// ─── Standalone function body lifters ────────────────────────────────────────
// These are free functions (not LiftCtx methods) so they can take &mut IrModule
// without conflicting with the parent context's borrow.

fn lift_function_body(
    module: &mut IrModule,
    name:   Option<&str>,
    params: &oxc_ast::ast::FormalParameters<'_>,
    body:   Option<&oxc_ast::ast::FunctionBody<'_>>,
) -> Result<FuncId> {
    let fname = name.unwrap_or("__anon__").to_string();
    let mut param_names = Vec::new();
    for (i, p) in params.items.iter().enumerate() {
        let name = match &p.pattern.kind {
            BindingPatternKind::BindingIdentifier(id) => id.name.as_str().to_string(),
            _ => format!("__param_{}__", i),
        };
        param_names.push(name);
    }

    let ir_params: Vec<(String, JsType)> = param_names
        .iter()
        .map(|n| (n.clone(), JsType::Any))
        .collect();

    let func_id = module.functions.alloc(Function::new(fname, ir_params));
    let mut ctx = LiftCtx::new(module, func_id);

    for (i, name) in param_names.iter().enumerate() {
        let slot = ctx.builder.local_new(name.clone());
        ctx.scope.define(name.clone(), slot);

        let p = &params.items[i];
        if !matches!(&p.pattern.kind, BindingPatternKind::BindingIdentifier(_)) {
            let val = ctx.builder.local_get(slot);
            ctx.destructure_binding(&p.pattern, val)?;
        }
    }

    if let Some(body) = body {
        ctx.scope.push();
        for stmt in &body.statements {
            ctx.lift_stmt(stmt)?;
        }
        ctx.scope.pop();
    }

    let undef = ctx.builder.const_undefined();
    ctx.builder.return_(Some(undef));
    Ok(func_id)
}

fn lift_arrow_body(
    module: &mut IrModule,
    arrow:  &oxc_ast::ast::ArrowFunctionExpression<'_>,
) -> Result<FuncId> {
    let mut param_names = Vec::new();
    for (i, p) in arrow.params.items.iter().enumerate() {
        let name = match &p.pattern.kind {
            BindingPatternKind::BindingIdentifier(id) => id.name.as_str().to_string(),
            _ => format!("__param_{}__", i),
        };
        param_names.push(name);
    }

    let ir_params: Vec<(String, JsType)> = param_names
        .iter()
        .map(|n| (n.clone(), JsType::Any))
        .collect();

    let func_id = module.functions.alloc(Function::new("__arrow__", ir_params));
    let mut ctx = LiftCtx::new(module, func_id);

    for (i, name) in param_names.iter().enumerate() {
        let slot = ctx.builder.local_new(name.clone());
        ctx.scope.define(name.clone(), slot);

        let p = &arrow.params.items[i];
        if !matches!(&p.pattern.kind, BindingPatternKind::BindingIdentifier(_)) {
            let val = ctx.builder.local_get(slot);
            ctx.destructure_binding(&p.pattern, val)?;
        }
    }

    if arrow.expression {
        for stmt in &arrow.body.statements {
            if let Statement::ExpressionStatement(e) = stmt {
                let val = ctx.lift_expr(&e.expression)?;
                ctx.builder.return_(Some(val));
            }
        }
    } else {
        ctx.scope.push();
        for stmt in &arrow.body.statements {
            ctx.lift_stmt(stmt)?;
        }
        ctx.scope.pop();
        let undef = ctx.builder.const_undefined();
        ctx.builder.return_(Some(undef));
    }

    Ok(func_id)
}

// ─── Operator Mapping ────────────────────────────────────────────────────────

fn map_binop(op: oxc_ast::ast::BinaryOperator) -> Result<BinOp> {
    use oxc_ast::ast::BinaryOperator::*;
    Ok(match op {
        Addition              => BinOp::Add,
        Subtraction           => BinOp::Sub,
        Multiplication        => BinOp::Mul,
        Division              => BinOp::Div,
        Remainder             => BinOp::Rem,
        Exponential           => BinOp::Pow,
        BitwiseAnd            => BinOp::BitAnd,
        BitwiseOR             => BinOp::BitOr,
        BitwiseXOR            => BinOp::BitXor,
        ShiftLeft             => BinOp::Shl,
        ShiftRight            => BinOp::Shr,
        ShiftRightZeroFill    => BinOp::UShr,
        Equality              => BinOp::Eq,
        StrictEquality        => BinOp::StrictEq,
        Inequality            => BinOp::Ne,
        StrictInequality      => BinOp::StrictNe,
        LessThan              => BinOp::Lt,
        LessEqualThan         => BinOp::Le,
        GreaterThan           => BinOp::Gt,
        GreaterEqualThan      => BinOp::Ge,
        In                    => BinOp::In,
        Instanceof            => BinOp::InstanceOf,
    })
}

fn map_unop(op: oxc_ast::ast::UnaryOperator) -> Result<UnOp> {
    use oxc_ast::ast::UnaryOperator::*;
    Ok(match op {
        UnaryNegation  => UnOp::Neg,
        LogicalNot     => UnOp::Not,
        BitwiseNot     => UnOp::BitNot,
        Typeof         => UnOp::TypeOf,
        Void           => UnOp::Void,
        Delete         => UnOp::Delete,
        UnaryPlus      => UnOp::Neg, // +x coerces to number; map to Neg then negate—handled at emit
    })
}
