use criterion::{criterion_group, criterion_main, Criterion};
use v12_runtime::V12Engine;
use v12_frontend::ParsedProgram;
use v12_lifter::Lifter;
use v12_emitter::Emitter;

fn bench_compile(c: &mut Criterion) {
    let source = r#"
        let total = 0;
        for (let j = 0; j < 100; j = j + 1) {
            total = total + j;
        }
    "#;

    c.bench_function("compile_for_loop", |b| {
        b.iter(|| {
            let parsed = ParsedProgram::from_source(source.to_string()).unwrap();
            let program = parsed.program();
            let ir = Lifter::new().lift_program(&program).unwrap();
            let _bytes = Emitter::new(&ir).emit().unwrap();
        });
    });
}

fn bench_eval(c: &mut Criterion) {
    let source = r#"
        let total = 0;
        for (let j = 0; j < 100; j = j + 1) {
            total = total + j;
        }
    "#;

    let engine = V12Engine::new().unwrap();

    c.bench_function("eval_for_loop", |b| {
        b.iter(|| {
            let _val = engine.eval(source).unwrap();
        });
    });
}

criterion_group!(benches, bench_compile, bench_eval);
criterion_main!(benches);
