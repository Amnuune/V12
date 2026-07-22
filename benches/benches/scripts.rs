#![allow(unused_crate_dependencies, missing_docs)]
use criterion::{criterion_group, criterion_main, Criterion};
use std::{path::Path, time::Duration};
use v12_emitter::Emitter;
use v12_frontend::ParsedProgram;
use v12_lifter::Lifter;
use v12_runtime::Runtime;

fn bench_scripts(c: &mut Criterion) {
    let scripts_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts");

    let mut scripts: Vec<_> = walkdir::WalkDir::new(&scripts_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.extension().is_some_and(|ext| ext == "js")
                && path
                    .file_name()
                    .is_some_and(|base| !base.display().to_string().starts_with('_'))
        })
        .collect();

    scripts.sort_by_key(|e| e.path().to_path_buf());

    for entry in scripts {
        let path = entry.path();
        let code = std::fs::read_to_string(path).unwrap();

        // Create a benchmark name from relative path
        let rel_path = path.strip_prefix(&scripts_dir).unwrap().with_extension("");
        let name = rel_path.display().to_string();

        let mut group = c.benchmark_group(&name);
        if rel_path.starts_with("v8-benches") {
            group.measurement_time(Duration::from_secs(20));
        }

        // If the script defines function main() but doesn't call it, append main();
        let full_code = if code.contains("function main") && !code.contains("main()") {
            format!("{}\nmain();", code)
        } else {
            code.clone()
        };

        let compile_result = (|| -> anyhow::Result<(Vec<u8>, Vec<String>)> {
            let parsed = ParsedProgram::from_source(&full_code)?;
            let program = parsed.program();
            let ir = Lifter::new().lift_program(&program)?;
            let string_table = ir.strings.clone();
            let wasm_bytes = Emitter::new(&ir).emit()?;
            Ok((wasm_bytes, string_table))
        })();

        let (wasm_bytes, string_table) = match compile_result {
            Ok(res) => res,
            Err(err) => {
                eprintln!("Skipping benchmark '{}' (compilation error: {})", name, err);
                group.finish();
                continue;
            }
        };

        let runtime = match Runtime::new() {
            Ok(r) => r,
            Err(err) => {
                eprintln!("Skipping benchmark '{}' (runtime creation error: {})", name, err);
                group.finish();
                continue;
            }
        };

        group.bench_function("Execution", |b| {
            b.iter(|| {
                let _ = runtime.run(&wasm_bytes, string_table.clone());
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_scripts);
criterion_main!(benches);
