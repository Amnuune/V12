# V12 Criterion Benchmark Results Walkthrough (By Subdirectory)

This document presents the detailed execution times of the V12 Engine benchmark run, structured exactly according to the subdirectories in [scripts](file:///home/nuun/Desktop/V12/benches/scripts).

To run all benchmarks across all directories:
```bash
cargo bench --bench scripts
```

---

## 1. basic
Microbenchmarks targeting general JS control flow, loops, and function calls.

**Run Command:**
```bash
cargo bench --bench scripts -- basic
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`basic/call-loop`** | 1.2380 ms | 100 |
| **`basic/closure`** | 1.7619 ms | 100 |
| **`basic/nested-loop`** | 1.5218 ms | 100 |

---

## 2. closures
Benchmarks testing closures creation and invocation performance.

**Run Command:**
```bash
cargo bench --bench scripts -- closures
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`closures/create`** | 1.7023 ms | 100 |
| **`closures/invoke`** | 2.0477 ms | 100 |

---

## 3. intl
ECMA-402 Internationalization benchmarks powered by ICU4X.

**Run Command:**
```bash
cargo bench --bench scripts -- intl
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`intl/collator-compare`** | 1.6388 ms | 100 |
| **`intl/collator-construction`** | 1.9220 ms | 100 |
| **`intl/datetimeformat-construction`** | 1.9178 ms | 100 |
| **`intl/datetimeformat-format`** | 1.6237 ms | 100 |
| **`intl/datetimeformat-with-options`** | 2.0766 ms | 100 |
| **`intl/datetimeformat_resolved_options`** | 1.5703 ms | 100 |
| **`intl/listformat-construction`** | 1.9338 ms | 100 |
| **`intl/listformat-format`** | 1.6299 ms | 100 |
| **`intl/numberformat-construction`** | 1.9274 ms | 100 |
| **`intl/numberformat-different-options`** | 2.1079 ms | 100 |
| **`intl/pluralrules-construction`** | 1.9233 ms | 100 |
| **`intl/pluralrules-select`** | 1.5951 ms | 100 |
| **`intl/segmenter-construction`** | 1.9250 ms | 100 |
| **`intl/segmenter-segment`** | 1.4289 ms | 100 |

---

## 4. json
JSON serialization benchmarks.

**Run Command:**
```bash
cargo bench --bench scripts -- json
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`json/stringify_circular`** | 2.1715 ms | 100 |
| **`json/stringify_deep`** | 3.3482 ms | 100 |

---

## 5. properties
Object property retrieval and setting performance benchmarks.

**Run Command:**
```bash
cargo bench --bench scripts -- properties
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`properties/access`** | 12.9430 ms | 100 |

---

## 6. prototypes
Prototype chain lookup performance.

**Run Command:**
```bash
cargo bench --bench scripts -- prototypes
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`prototypes/chain`** | 2.3703 ms | 100 |

---

## 7. strings
Common JS string operations.

**Run Command:**
```bash
cargo bench --bench scripts -- strings
```

| Benchmark | Mean Execution Time | Sample Size |
|---|---|---|
| **`strings/concat`** | 1.8143 ms | 100 |
| **`strings/replace`** | 2.1662 ms | 100 |
| **`strings/slice`** | 1.7987 ms | 100 |
| **`strings/split`** | 1.3568 ms | 100 |

---

## 8. v8-benches
The V8 octene/benchmark suite containing complex programs. Both sample sizes are shown here.

**Run Command:**
```bash
cargo bench --bench scripts -- v8-benches
```

| Benchmark | Time (10 Samples) | Time (100 Samples) | Comparison (100 vs 10) |
|---|---|---|---|
| **`v8-benches/crypto`** | 80.1320 ms | 71.5370 ms | 10.73% faster |
| **`v8-benches/deltablue`** | 25.5230 ms | 25.1340 ms | 1.52% faster |
| **`v8-benches/earley-boyer`** | 84.6110 ms | 81.3320 ms | 3.88% faster |
| **`v8-benches/navier-stokes`** | 18.5610 ms | 17.3290 ms | 6.64% faster |
| **`v8-benches/raytrace`** | 36.0330 ms | 38.4100 ms | 6.60% slower |
| **`v8-benches/regexp`** | 84.5140 ms | 88.2100 ms | 4.37% slower |
| **`v8-benches/richards`** | 15.6890 ms | 16.9230 ms | 7.87% slower |
| **`v8-benches/splay`** | 12.7390 ms | 12.3680 ms | 2.91% faster |
