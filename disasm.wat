warning: unused import: `id_arena::Arena`
 --> crates/v12-ir/src/builder.rs:3:5
  |
3 | use id_arena::Arena;
  |     ^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused import: `Function`
 --> crates/v12-ir/src/builder.rs:5:41
  |
5 |     BasicBlock, BlockId, Const, FuncId, Function, Instruction, IrModule,
  |                                         ^^^^^^^^

warning: unused import: `types::*`
  --> crates/v12-ir/src/lib.rs:13:9
   |
13 | pub use types::*;
   |         ^^^^^^^^

warning: method `alloc_value` is never used
  --> crates/v12-ir/src/builder.rs:48:8
   |
24 | impl<'m> IrBuilder<'m> {
   | ---------------------- method in this implementation
...
48 |     fn alloc_value(&mut self, ty: JsType, kind: ValueKind) -> ValueId {
   |        ^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `v12-ir` (lib) generated 4 warnings (run `cargo fix --lib -p v12-ir` to apply 3 suggestions)
warning: unused variable: `fid`
   --> crates/v12-emitter/src/emitter.rs:103:18
    |
103 |         for (i, &fid) in func_ids.iter().enumerate() {
    |                  ^^^ help: if this is intentional, prefix it with an underscore: `_fid`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `p`
   --> crates/v12-emitter/src/emitter.rs:689:14
    |
689 |         for (p, &(_, lh_depth, cond_bb, after_bb, update_bb)) in loop_s...
    |              ^ help: if this is intentional, prefix it with an underscore: `_p`

warning: `v12-emitter` (lib) generated 2 warnings (run `cargo fix --lib -p v12-emitter` to apply 2 suggestions)
warning: unused import: `tokio::sync::mpsc`
 --> crates/v12-runtime/src/event_loop.rs:9:5
  |
9 | use tokio::sync::mpsc;
  |     ^^^^^^^^^^^^^^^^^
  |
  = note: `#[warn(unused_imports)]` (part of `#[warn(unused)]`) on by default

warning: unused imports: `Arc` and `RwLock`
  --> crates/v12-runtime/src/runtime.rs:10:17
   |
10 | use std::sync::{Arc, RwLock};
   |                 ^^^  ^^^^^^

warning: unused import: `crate::event_loop::EventLoop`
  --> crates/v12-runtime/src/runtime.rs:17:5
   |
17 | use crate::event_loop::EventLoop;
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused import: `JsObjectInner`
  --> crates/v12-runtime/src/runtime.rs:18:23
   |
18 | use crate::js_value::{JsObjectInner, JsValue};
   |                       ^^^^^^^^^^^^^

warning: unused imports: `Arc` and `Mutex`
 --> crates/v12-runtime/src/rt_functions.rs:6:17
  |
6 | use std::sync::{Arc, Mutex};
  |                 ^^^  ^^^^^

warning: unused imports: `Caller`, `StoreContextMut`, and `Val`
 --> crates/v12-runtime/src/rt_functions.rs:7:16
  |
7 | use wasmtime::{Caller, ExternRef, Rooted, StoreContextMut, Val};
  |                ^^^^^^                     ^^^^^^^^^^^^^^^  ^^^

warning: unused import: `JsObjectInner`
 --> crates/v12-runtime/src/rt_functions.rs:8:23
  |
8 | use crate::js_value::{JsObjectInner, JsValue};
  |                       ^^^^^^^^^^^^^

warning: unused import: `HostFn`
 --> crates/v12-runtime/src/builtins.rs:1:32
  |
1 | use crate::js_value::{JsValue, HostFn, GLOBAL_PROTOTYPES};
  |                                ^^^^^^

warning: unused import: `std::collections::HashMap`
 --> crates/v12-runtime/src/builtins.rs:3:5
  |
3 | use std::collections::HashMap;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused variable: `a`
   --> crates/v12-runtime/src/runtime.rs:367:13
    |
367 |             a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
    |             ^ help: if this is intentional, prefix it with an underscore: `_a`
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `b`
   --> crates/v12-runtime/src/runtime.rs:367:43
    |
367 |             a: Option<Rooted<ExternRef>>, b: Option<Rooted<ExternRef>>|
    |                                           ^ help: if this is intentional, prefix it with an underscore: `_b`

warning: `v12-runtime` (lib) generated 11 warnings (run `cargo fix --lib -p v12-runtime` to apply 11 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.19s
     Running `target/debug/disasm`
(module
  (type (;0;) (func (result externref)))
  (type (;1;) (func (param externref) (result externref)))
  (type (;2;) (func (param externref externref) (result externref)))
  (type (;3;) (func (param externref externref externref) (result externref)))
  (type (;4;) (func (param externref externref i32) (result externref)))
  (type (;5;) (func (param i32) (result externref)))
  (type (;6;) (func (param f64) (result externref)))
  (type (;7;) (func (param externref) (result i32)))
  (type (;8;) (func (result i32)))
  (import "v12rt" "rt_undefined" (func (;0;) (type 0)))
  (import "v12rt" "rt_null" (func (;1;) (type 0)))
  (import "v12rt" "rt_true" (func (;2;) (type 0)))
  (import "v12rt" "rt_false" (func (;3;) (type 0)))
  (import "v12rt" "rt_object_new" (func (;4;) (type 0)))
  (import "v12rt" "rt_array_new" (func (;5;) (type 0)))
  (import "v12rt" "rt_int" (func (;6;) (type 5)))
  (import "v12rt" "rt_float" (func (;7;) (type 6)))
  (import "v12rt" "rt_string" (func (;8;) (type 5)))
  (import "v12rt" "rt_truthy_i32" (func (;9;) (type 7)))
  (import "v12rt" "rt_neg" (func (;10;) (type 1)))
  (import "v12rt" "rt_not" (func (;11;) (type 1)))
  (import "v12rt" "rt_bit_not" (func (;12;) (type 1)))
  (import "v12rt" "rt_typeof" (func (;13;) (type 1)))
  (import "v12rt" "rt_void" (func (;14;) (type 1)))
  (import "v12rt" "rt_delete" (func (;15;) (type 1)))
  (import "v12rt" "rt_pre_inc" (func (;16;) (type 1)))
  (import "v12rt" "rt_pre_dec" (func (;17;) (type 1)))
  (import "v12rt" "rt_post_inc" (func (;18;) (type 1)))
  (import "v12rt" "rt_post_dec" (func (;19;) (type 1)))
  (import "v12rt" "rt_truthy" (func (;20;) (type 1)))
  (import "v12rt" "rt_to_number" (func (;21;) (type 1)))
  (import "v12rt" "rt_to_string" (func (;22;) (type 1)))
  (import "v12rt" "rt_console_log" (func (;23;) (type 1)))
  (import "v12rt" "rt_console_error" (func (;24;) (type 1)))
  (import "v12rt" "rt_console_warn" (func (;25;) (type 1)))
  (import "v12rt" "rt_eval_js" (func (;26;) (type 1)))
  (import "v12rt" "rt_global_get" (func (;27;) (type 1)))
  (import "v12rt" "rt_push_arg" (func (;28;) (type 1)))
  (import "v12rt" "rt_new" (func (;29;) (type 1)))
  (import "v12rt" "rt_js_new" (func (;30;) (type 4)))
  (import "v12rt" "rt_js_await" (func (;31;) (type 1)))
  (import "v12rt" "rt_throw" (func (;32;) (type 1)))
  (import "v12rt" "rt_add" (func (;33;) (type 2)))
  (import "v12rt" "rt_sub" (func (;34;) (type 2)))
  (import "v12rt" "rt_mul" (func (;35;) (type 2)))
  (import "v12rt" "rt_div" (func (;36;) (type 2)))
  (import "v12rt" "rt_rem" (func (;37;) (type 2)))
  (import "v12rt" "rt_pow" (func (;38;) (type 2)))
  (import "v12rt" "rt_bit_and" (func (;39;) (type 2)))
  (import "v12rt" "rt_bit_or" (func (;40;) (type 2)))
  (import "v12rt" "rt_bit_xor" (func (;41;) (type 2)))
  (import "v12rt" "rt_shl" (func (;42;) (type 2)))
  (import "v12rt" "rt_shr" (func (;43;) (type 2)))
  (import "v12rt" "rt_ushr" (func (;44;) (type 2)))
  (import "v12rt" "rt_eq" (func (;45;) (type 2)))
  (import "v12rt" "rt_strict_eq" (func (;46;) (type 2)))
  (import "v12rt" "rt_ne" (func (;47;) (type 2)))
  (import "v12rt" "rt_strict_ne" (func (;48;) (type 2)))
  (import "v12rt" "rt_lt" (func (;49;) (type 2)))
  (import "v12rt" "rt_le" (func (;50;) (type 2)))
  (import "v12rt" "rt_gt" (func (;51;) (type 2)))
  (import "v12rt" "rt_ge" (func (;52;) (type 2)))
  (import "v12rt" "rt_and" (func (;53;) (type 2)))
  (import "v12rt" "rt_or" (func (;54;) (type 2)))
  (import "v12rt" "rt_nullish" (func (;55;) (type 2)))
  (import "v12rt" "rt_in" (func (;56;) (type 2)))
  (import "v12rt" "rt_instanceof" (func (;57;) (type 2)))
  (import "v12rt" "rt_prop_get" (func (;58;) (type 2)))
  (import "v12rt" "rt_array_get" (func (;59;) (type 2)))
  (import "v12rt" "rt_global_set" (func (;60;) (type 2)))
  (import "v12rt" "rt_prop_set" (func (;61;) (type 3)))
  (import "v12rt" "rt_array_set" (func (;62;) (type 3)))
  (import "v12rt" "rt_call" (func (;63;) (type 4)))
  (import "v12rt" "rt_has_exception" (func (;64;) (type 8)))
  (import "v12rt" "rt_get_exception" (func (;65;) (type 0)))
  (import "v12rt" "rt_get_arg" (func (;66;) (type 5)))
  (import "v12rt" "rt_get_this" (func (;67;) (type 0)))
  (func (;68;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref)
    i32.const 69
    call 6
    local.set 2
    i32.const 2
    call 8
    local.set 3
    local.get 3
    local.get 2
    call 60
    local.set 4
    call 4
    local.set 5
    i32.const 4
    call 8
    local.set 6
    i32.const 70
    call 6
    local.set 7
    local.get 5
    local.get 6
    local.get 7
    call 61
    drop
    i32.const 2
    call 8
    local.set 8
    local.get 8
    call 27
    local.set 9
    i32.const 6
    call 8
    local.set 10
    local.get 9
    local.get 10
    local.get 5
    call 61
    drop
    i32.const 71
    call 6
    local.set 11
    i32.const 7
    call 8
    local.set 12
    local.get 12
    local.get 11
    call 60
    local.set 13
    i32.const 72
    call 6
    local.set 14
    i32.const 7
    call 8
    local.set 15
    local.get 15
    call 27
    local.set 16
    i32.const 10
    call 8
    local.set 17
    local.get 16
    local.get 17
    local.get 14
    call 61
    drop
    i32.const 73
    call 6
    local.set 18
    i32.const 7
    call 8
    local.set 19
    local.get 19
    call 27
    local.set 20
    i32.const 12
    call 8
    local.set 21
    local.get 20
    local.get 21
    local.get 18
    call 61
    drop
    i32.const 0
    call 6
    local.set 22
    local.get 22
    local.set 0
    block ;; label = @1
      loop ;; label = @2
        local.get 0
        local.set 23
        i32.const 0
        call 6
        local.set 24
        local.get 23
        local.get 24
        call 50
        local.set 25
        local.get 25
        call 20
        local.set 26
        local.get 26
        call 9
        i32.eqz
        br_if 1 (;@1;)
        i32.const 0
        call 6
        local.set 27
        local.get 27
        local.set 1
        local.get 0
        local.set 28
        local.get 28
        call 18
        local.set 29
        local.get 29
        local.set 0
        br 0 (;@2;)
      end
    end
    call 3
    local.set 30
    i32.const 14
    call 8
    local.set 31
    local.get 31
    local.get 30
    call 60
    local.set 32
    i32.const 0
    call 6
    local.set 33
    local.get 33
    local.set 0
    block ;; label = @1
      loop ;; label = @2
        local.get 1
        local.set 34
        i32.const 1
        call 6
        local.set 35
        local.get 34
        local.get 35
        call 50
        local.set 36
        local.get 36
        call 20
        local.set 37
        local.get 37
        call 9
        i32.eqz
        br_if 1 (;@1;)
        local.get 1
        local.set 38
        i32.const 0
        call 6
        local.set 39
        local.get 38
        local.get 39
        call 46
        local.set 40
        local.get 40
        call 20
        local.set 41
        local.get 41
        call 9
        if ;; label = @3
        else
          i32.const 2
          call 8
          local.set 44
          local.get 44
          call 27
          local.set 45
          i32.const 13
          call 8
          local.set 46
          local.get 46
          call 28
          drop
          local.get 45
          call 0
          i32.const 1
          call 30
          local.set 47
          local.get 47
          call 32
          unreachable
        end
        local.get 1
        local.set 42
        local.get 42
        call 18
        local.set 43
        local.get 43
        local.set 1
        br 0 (;@2;)
      end
    end
    block ;; label = @1
      loop ;; label = @2
        local.get 0
        local.set 48
        i32.const 1
        call 6
        local.set 49
        local.get 48
        local.get 49
        call 50
        local.set 50
        local.get 50
        call 20
        local.set 51
        local.get 51
        call 9
        i32.eqz
        br_if 1 (;@1;)
        i32.const 0
        call 6
        local.set 52
        local.get 52
        local.set 1
        local.get 0
        local.set 53
        local.get 53
        call 18
        local.set 54
        local.get 54
        local.set 0
        br 0 (;@2;)
      end
    end
    i32.const 14
    call 8
    local.set 55
    local.get 55
    call 27
    local.set 56
    call 2
    local.set 57
    local.get 56
    local.get 57
    call 48
    local.set 58
    local.get 58
    call 20
    local.set 59
    local.get 59
    call 9
    if ;; label = @1
      i32.const 2
      call 8
      local.set 75
      local.get 75
      call 27
      local.set 76
      i32.const 16
      call 8
      local.set 77
      local.get 77
      call 28
      drop
      local.get 76
      call 0
      i32.const 1
      call 30
      local.set 78
      local.get 78
      call 32
      unreachable
    else
    end
    block ;; label = @1
      loop ;; label = @2
        local.get 1
        local.set 60
        i32.const 1
        call 6
        local.set 61
        local.get 60
        local.get 61
        call 50
        local.set 62
        local.get 62
        call 20
        local.set 63
        local.get 63
        call 9
        i32.eqz
        br_if 1 (;@1;)
        local.get 1
        local.set 64
        i32.const 0
        call 6
        local.set 65
        local.get 64
        local.get 65
        call 46
        local.set 66
        local.get 66
        call 20
        local.set 67
        local.get 67
        call 9
        if ;; label = @3
        else
          call 2
          local.set 70
          i32.const 14
          call 8
          local.set 71
          local.get 71
          local.get 70
          call 60
          local.set 72
        end
        local.get 1
        local.set 68
        local.get 68
        call 18
        local.set 69
        local.get 69
        local.set 1
        br 0 (;@2;)
      end
    end
    i32.const 15
    call 8
    local.set 73
    local.get 73
    call 27
    local.set 74
    call 0
    local.set 79
    local.get 79
    return
    call 0
    return
  )
  (func (;69;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref)
    i32.const 0
    call 66
    local.set 0
    local.get 0
    local.set 1
    i32.const 0
    call 8
    local.set 2
    local.get 1
    local.get 2
    call 54
    local.set 3
    call 67
    local.set 4
    i32.const 1
    call 8
    local.set 5
    local.get 4
    local.get 5
    local.get 3
    call 61
    drop
    i32.const 2
    call 8
    local.set 6
    call 67
    local.set 7
    i32.const 3
    call 8
    local.set 8
    local.get 7
    local.get 8
    local.get 6
    call 61
    drop
    call 0
    local.set 9
    local.get 9
    return
    call 0
    return
  )
  (func (;70;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref)
    call 67
    local.set 0
    i32.const 3
    call 8
    local.set 1
    local.get 0
    local.get 1
    call 58
    local.set 2
    i32.const 5
    call 8
    local.set 3
    local.get 2
    local.get 3
    call 33
    local.set 4
    call 67
    local.set 5
    i32.const 1
    call 8
    local.set 6
    local.get 5
    local.get 6
    call 58
    local.set 7
    local.get 4
    local.get 7
    call 33
    local.set 8
    local.get 8
    return
    call 0
    return
  )
  (func (;71;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref)
    i32.const 0
    call 66
    local.set 0
    i32.const 1
    call 66
    local.set 1
    local.get 0
    local.set 2
    local.get 2
    call 11
    local.set 3
    local.get 3
    call 20
    local.set 4
    local.get 4
    call 9
    if ;; label = @1
      i32.const 2
      call 8
      local.set 5
      local.get 5
      call 27
      local.set 6
      local.get 1
      local.set 7
      local.get 7
      call 28
      drop
      local.get 6
      call 0
      i32.const 1
      call 30
      local.set 8
      local.get 8
      call 32
      unreachable
    else
    end
    call 0
    local.set 9
    local.get 9
    return
    call 0
    return
  )
  (func (;72;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref)
    i32.const 0
    call 66
    local.set 0
    i32.const 1
    call 66
    local.set 1
    i32.const 2
    call 66
    local.set 2
    local.get 0
    local.set 3
    local.get 1
    local.set 4
    local.get 3
    local.get 4
    call 48
    local.set 5
    local.get 5
    call 20
    local.set 6
    local.get 6
    call 9
    if ;; label = @1
      i32.const 2
      call 8
      local.set 7
      local.get 7
      call 27
      local.set 8
      call 0
      local.set 9
      i32.const 8
      call 8
      local.set 10
      local.get 9
      local.get 10
      call 33
      local.set 11
      local.get 1
      local.set 12
      local.get 11
      local.get 12
      call 33
      local.set 13
      i32.const 9
      call 8
      local.set 14
      local.get 13
      local.get 14
      call 33
      local.set 15
      local.get 0
      local.set 16
      local.get 15
      local.get 16
      call 33
      local.set 17
      local.get 17
      call 28
      drop
      local.get 8
      call 0
      i32.const 1
      call 30
      local.set 18
      local.get 18
      call 32
      unreachable
    else
    end
    call 0
    local.set 19
    local.get 19
    return
    call 0
    return
  )
  (func (;73;) (type 0) (result externref)
    (local externref externref externref externref externref externref externref externref externref externref externref externref externref externref externref)
    i32.const 0
    call 66
    local.set 0
    i32.const 1
    call 66
    local.set 1
    i32.const 2
    call 66
    local.set 2
    block ;; label = @1
      block ;; label = @2
        call 0
        local.set 4
        local.get 1
        local.set 5
        local.get 5
        local.get 4
        i32.const 0
        call 63
        local.set 6
        call 64
        br_if 0 (;@2;)
      end
      br 0 (;@1;)
      call 65
      local.set 7
      local.get 7
      local.set 3
      call 0
      return
    end
    i32.const 2
    call 8
    local.set 8
    local.get 8
    call 27
    local.set 9
    local.get 2
    local.set 10
    i32.const 11
    call 8
    local.set 11
    local.get 10
    local.get 11
    call 54
    local.set 12
    local.get 12
    call 28
    drop
    local.get 9
    call 0
    i32.const 1
    call 30
    local.set 13
    local.get 13
    call 32
    unreachable
    call 0
    return
  )
  (export "__main__" (func 68))
  (export "func_68" (func 68))
  (export "func_69" (func 69))
  (export "func_70" (func 70))
  (export "func_71" (func 71))
  (export "func_72" (func 72))
  (export "func_73" (func 73))
)

RUN RESULT: Err(Test262Error: #1: Check continue statement for automatic semicolon insertion)
