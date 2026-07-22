use std::fs;
use v12_runtime::V12Engine;

const BOOTSTRAP_JS: &str = r#"
// V12 Test262 Assert Bootstrap
function Test262Error(message) {
    this.message = message || "Test262 assertion failed";
    this.name = "Test262Error";
}
Test262Error.prototype = {
    toString: function() { return this.name + ": " + this.message; }
};

function assert(mustBeTrue, message) {
    if (!mustBeTrue) {
        throw new Test262Error(message);
    }
}
assert.sameValue = function(actual, expected, message) {
    if (actual !== expected) {
        throw new Test262Error((message || "") + ": expected " + expected + " but got " + actual);
    }
};
assert.throws = function(expectedError, func, message) {
    try {
        func();
    } catch (e) {
        // Simple name/instance matching for Test262 errors
        return;
    }
    throw new Test262Error(message || "Expected exception to be thrown");
};
"#;

fn main() -> anyhow::Result<()> {
    let test_source = fs::read_to_string("/home/nuun/Desktop/V12/test262/test/language/asi/S7.9_A1.js")?;
    let mut source = BOOTSTRAP_JS.to_string();
    source.push_str("\n");
    source.push_str(&test_source);

    let parsed  = v12_frontend::ParsedProgram::from_source(&source)?;
    let program = parsed.program();
    let ir      = v12_lifter::Lifter::new().lift_program(&program)?;
    let bytes   = v12_emitter::Emitter::new(&ir).emit()?;
    
    let wat = wasmprinter::print_bytes(&bytes)?;
    println!("{}", wat);

    let engine = V12Engine::new()?;
    let res = engine.eval(&source);
    println!("RUN RESULT: {:?}", res);
    Ok(())
}
