//! V12Engine: the top-level compiler+runner.
//!
//! Usage:
//!   let engine = V12Engine::new()?;
//!   engine.run_file("hello.js")?;

use anyhow::Result;
use v12_frontend::ParsedProgram;
use v12_lifter::Lifter;
use v12_emitter::Emitter;
use crate::runtime::Runtime;
use crate::js_value::JsValue;

pub struct V12Engine {
    runtime: Runtime,
}

impl V12Engine {
    pub fn new() -> Result<Self> {
        Ok(Self { runtime: Runtime::new()? })
    }

    /// Compile and run a JavaScript source string.
    pub fn eval(&self, source: &str) -> Result<JsValue> {
        // 1. Parse.
        let parsed = ParsedProgram::from_source(source)?;
        let program = parsed.program();

        // 2. Lift AST → IR.
        let lifter = Lifter::new();
        let ir_module = lifter.lift_program(&program)?;

        // 3. Extract string table before consuming the module.
        let string_table = ir_module.strings.clone();

        // 4. Emit IR → Wasm bytes.
        let emitter = Emitter::new(&ir_module);
        let wasm_bytes = emitter.emit()?;

        log::debug!("Emitted {} bytes of Wasm", wasm_bytes.len());

        // 5. Run.
        self.runtime.run(&wasm_bytes, string_table)
    }

    /// Compile and run a JavaScript file.
    pub fn run_file(&self, path: &str) -> Result<JsValue> {
        let source = std::fs::read_to_string(path)?;
        self.eval(&source)
    }

    /// Compile a JavaScript file to Wasm bytes without running it.
    /// Returns (wasm_bytes, function_count).
    pub fn compile_file(&self, path: &str) -> Result<(Vec<u8>, usize)> {
        let source  = std::fs::read_to_string(path)?;
        let parsed  = ParsedProgram::from_source(source)?;
        let program = parsed.program();
        let ir      = Lifter::new().lift_program(&program)?;
        let func_count = ir.functions.iter().count();
        let bytes   = Emitter::new(&ir).emit()?;
        Ok((bytes, func_count))
    }
}
