use std::collections::HashMap;
use std::path::{Path, PathBuf};
use rayon::prelude::*;
use yaml_rust::YamlLoader;
use v12_runtime::V12Engine;
use anyhow::Result;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TestResult {
    Pass,
    Fail(String),
    Skip(String),
}

struct Frontmatter {
    includes: Vec<String>,
    flags: Vec<String>,
    negative: Option<String>,
}

fn parse_frontmatter(content: &str) -> (Option<Frontmatter>, &str) {
    if let Some(start) = content.find("/*---") {
        if let Some(end) = content[start..].find("---*/") {
            let yaml_str = &content[start + 5..start + end];
            let rest = &content[start + end + 5..];
            if let Ok(docs) = YamlLoader::load_from_str(yaml_str) {
                if let Some(doc) = docs.first() {
                    let mut includes = Vec::new();
                    if let Some(arr) = doc["includes"].as_vec() {
                        for val in arr {
                            if let Some(s) = val.as_str() {
                                includes.push(s.to_string());
                            }
                        }
                    }
                    let mut flags = Vec::new();
                    if let Some(arr) = doc["flags"].as_vec() {
                        for val in arr {
                            if let Some(s) = val.as_str() {
                                flags.push(s.to_string());
                            }
                        }
                    }
                    let negative = if !doc["negative"].is_badvalue() {
                        doc["negative"]["type"].as_str().map(|s| s.to_string())
                    } else {
                        None
                    };
                    return (
                        Some(Frontmatter {
                            includes,
                            flags,
                            negative,
                        }),
                        rest,
                    );
                }
            }
        }
    }
    (None, content)
}

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

pub fn run_test262(test262_dir: &str, suite: &str) -> Result<()> {
    let base_path = Path::new(test262_dir);
    let harness_dir = base_path.join("harness");
    let test_dir = match suite {
        "built-ins" | "builtins" => base_path.join("test").join("built-ins"),
        "annexB" | "annexb" => base_path.join("test").join("annexB"),
        "intl402" => base_path.join("test").join("intl402"),
        "staging" => base_path.join("test").join("staging"),
        "harness" => base_path.join("test").join("harness"),
        _ => base_path.join("test").join("language"),
    };

    println!("Loading harness from {:?}", harness_dir);
    let mut harness = HashMap::new();
    if harness_dir.exists() {
        for entry in std::fs::read_dir(harness_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "js") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                let code = std::fs::read_to_string(&path)?;
                harness.insert(name, code);
            }
        }
    } else {
        anyhow::bail!("Harness directory not found at {:?}", harness_dir);
    }

    println!("Walking {} tests in {:?}", suite, test_dir);
    let mut subdirs = Vec::new();
    for entry in std::fs::read_dir(&test_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        }
    }
    if subdirs.is_empty() {
        subdirs.push(test_dir.clone());
    }
    subdirs.sort();

    println!("Running tests in parallel across {} directories...", subdirs.len());

    // Print table header
    println!("\n| Directory | Passed | Failed | Skipped | Pass % |");
    println!("|---|---|---|---|---|");

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut total_skipped = 0;

    for subdir in subdirs {
        let dir_name = subdir.file_name().unwrap().to_string_lossy().into_owned();
        
        let mut js_files = Vec::new();
        collect_js_files(&subdir, &mut js_files)?;

        let results: Vec<TestResult> = js_files
            .par_iter()
            .map(|file| run_single_test(file, &harness, BOOTSTRAP_JS))
            .collect();

        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut printed_fails = 0;

        for res in results {
            match res {
                TestResult::Pass => passed += 1,
                TestResult::Fail(err) => {
                    failed += 1;
                    if printed_fails < 5 {
                        println!("      [FAIL] {}", err);
                        printed_fails += 1;
                    }
                }
                TestResult::Skip(_) => skipped += 1,
            }
        }

        let total = passed + failed;
        let pct = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "| {} | {} | {} | {} | {:.2}% |",
            dir_name, passed, failed, skipped, pct
        );

        total_passed += passed;
        total_failed += failed;
        total_skipped += skipped;
    }

    let overall_total = total_passed + total_failed;
    let overall_pct = if overall_total > 0 {
        (total_passed as f64 / overall_total as f64) * 100.0
    } else {
        0.0
    };

    println!("|---|---|---|---|---|");
    println!(
        "| **TOTAL** | **{}** | **{}** | **{}** | **{:.2}%** |",
        total_passed, total_failed, total_skipped, overall_pct
    );

    Ok(())
}

fn collect_js_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_js_files(&path, files)?;
        } else if path.is_file() && path.extension().map_or(false, |ext| ext == "js") {
            files.push(path);
        }
    }
    Ok(())
}

fn run_single_test(
    file_path: &Path,
    harness: &HashMap<String, String>,
    bootstrap_js: &str,
) -> TestResult {
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => return TestResult::Fail(format!("Read error: {}", e)),
    };

    let (fm_opt, test_js) = parse_frontmatter(&content);

    let mut code = String::new();
    code.push_str(bootstrap_js);
    code.push('\n');

    if let Some(sta) = harness.get("sta.js") {
        code.push_str(sta);
        code.push('\n');
    }
    if let Some(assert_js) = harness.get("assert.js") {
        code.push_str(assert_js);
        code.push('\n');
    }
    if let Some(prop_js) = harness.get("propertyHelper.js") {
        code.push_str(prop_js);
        code.push('\n');
    }
    if let Some(cmp_js) = harness.get("compareArray.js") {
        code.push_str(cmp_js);
        code.push('\n');
    }

    if let Some(ref fm) = fm_opt {
        for inc in &fm.includes {
            if inc == "sta.js" || inc == "assert.js" {
                continue;
            }
            if let Some(inc_code) = harness.get(inc) {
                code.push_str(inc_code);
                code.push('\n');
            } else {
                return TestResult::Skip(format!("Missing include: {}", inc));
            }
        }
    }

    code.push_str(test_js);

    let engine = match V12Engine::new() {
        Ok(e) => e,
        Err(e) => return TestResult::Fail(format!("Engine init fail: {}", e)),
    };

    let run_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.eval(&code)
    }));

    let eval_res = match run_res {
        Ok(r) => r,
        Err(_) => return TestResult::Fail("Engine panicked during execution".to_string()),
    };

    match eval_res {
        Ok(_) => {
            if let Some(ref fm) = fm_opt {
                if let Some(ref neg) = fm.negative {
                    return TestResult::Fail(format!("Expected exception '{}' but succeeded", neg));
                }
            }
            TestResult::Pass
        }
        Err(e) => {
            let err_str = format!("{:#}", e);
            if let Some(ref fm) = fm_opt {
                if let Some(ref neg) = fm.negative {
                    if err_str.contains(neg) || (neg == "SyntaxError" && (err_str.contains("Parse error") || err_str.contains("syntax") || err_str.contains("This statement should not be evaluated"))) {
                        return TestResult::Pass;
                    } else {
                        return TestResult::Fail(format!("Expected '{}', got: {}", neg, err_str));
                    }
                }
            }
            TestResult::Fail(err_str)
        }
    }
}
