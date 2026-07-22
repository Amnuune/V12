const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const mode = process.argv.includes('--jitless') ? 'jitless' : 'jit';
const outputArgIdx = process.argv.indexOf('--output');
const outputPath = outputArgIdx !== -1 ? process.argv[outputArgIdx + 1] : null;

const scriptsDir = '/home/nuun/Desktop/V12/benches/scripts';
const scratchDir = '/home/nuun/.gemini/antigravity-ide/brain/38b65025-b6e2-4ee3-9dc0-fbcc06dbb6db/scratch';

if (!fs.existsSync(scratchDir)) {
  fs.mkdirSync(scratchDir, { recursive: true });
}

// Get all JS benchmark files
function getFiles(dir) {
  let results = [];
  const list = fs.readdirSync(dir);
  list.forEach(file => {
    const fullPath = path.join(dir, file);
    const stat = fs.statSync(fullPath);
    if (stat && stat.isDirectory()) {
      results = results.concat(getFiles(fullPath));
    } else if (file.endsWith('.js') && !file.startsWith('_')) {
      results.push(fullPath);
    }
  });
  return results;
}

const files = getFiles(scriptsDir).sort();

// We will group benchmarks by directory
const groups = {};
files.forEach(file => {
  const relPath = path.relative(scriptsDir, file);
  const parts = relPath.split(path.sep);
  const dirName = parts[0];
  const benchName = relPath.replace(/\.js$/, '');
  
  if (!groups[dirName]) {
    groups[dirName] = [];
  }
  groups[dirName].push({
    filePath: file,
    benchName: benchName
  });
});

const reportData = {};

console.log(`Running benchmarks in mode: ${mode.toUpperCase()}`);

const execNodeCommand = mode === 'jitless' ? 'node --jitless' : 'node';

Object.keys(groups).forEach(dirName => {
  console.log(`\nProcessing directory: ${dirName}`);
  reportData[dirName] = [];

  groups[dirName].forEach(bench => {
    const isV8Bench = dirName === 'v8-benches';
    const filePath = bench.filePath;
    const benchName = bench.benchName;

    console.log(`  Benchmarking ${benchName}...`);

    if (isV8Bench) {
      // Run for 10 samples
      const time10 = runSingleBench(filePath, true, 10);
      // Run for 100 samples
      const time100 = runSingleBench(filePath, true, 100);

      reportData[dirName].push({
        benchName,
        time10,
        time100
      });
    } else {
      // Run for 100 samples
      const meanTime = runSingleBench(filePath, false, 100);
      reportData[dirName].push({
        benchName,
        meanTime
      });
    }
  });
});

// Format the final report
generateReport();

function runSingleBench(filePath, isV8Bench, sampleSize) {
  const originalCode = fs.readFileSync(filePath, 'utf8');

  // Build the runner code to append
  const runnerCode = `
// Runner Code
const { performance } = require('perf_hooks');
const isV8Bench = ${isV8Bench};
const sampleSize = ${sampleSize};

if (isV8Bench) {
  globalThis.Date = function() {};
}

if (typeof print === 'undefined') {
  globalThis.print = console.log;
}

// Warmup
const warmupCount = isV8Bench ? 1 : 5;
for (let i = 0; i < warmupCount; i++) {
  try {
    main();
  } catch (e) {}
}

// Measurement
const samples = [];
for (let i = 0; i < sampleSize; i++) {
  const t0 = performance.now();
  try {
    main();
  } catch (e) {
    console.error(e);
  }
  const t1 = performance.now();
  samples.push(t1 - t0);
}

console.log(JSON.stringify({ samples }));
process.exit(0);
`;

  const combinedCode = originalCode + '\n' + runnerCode;
  const tempFile = path.join(scratchDir, `temp_runner_${Math.random().toString(36).substring(2)}.js`);
  fs.writeFileSync(tempFile, combinedCode, 'utf8');

  try {
    const output = execSync(`${execNodeCommand} ${tempFile}`, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    const match = output.match(/\{"samples":\[.*\]\}/);
    if (!match) {
      throw new Error(`Invalid output from runner: ${output}`);
    }
    const result = JSON.parse(match[0]);
    const mean = result.samples.reduce((a, b) => a + b, 0) / result.samples.length;
    return mean;
  } catch (err) {
    console.error(`Error running ${filePath}:`, err.message);
    return NaN;
  } finally {
    try {
      if (fs.existsSync(tempFile)) {
        fs.unlinkSync(tempFile);
      }
    } catch (e) {}
  }
}

function generateReport() {
  const nowStr = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
  
  let md = `Created At: ${nowStr}
Completed At: ${nowStr}
File Path: \`file:///home/nuun/Desktop/V12/V12-SPEED.md\`

# V8 ${mode.toUpperCase()} Criterion Benchmark Results Walkthrough (By Subdirectory)

This document presents the detailed execution times of the V8 ${mode.toUpperCase()} benchmark run, structured exactly according to the subdirectories in [scripts](file:///home/nuun/Desktop/V12/benches/scripts).

To run all benchmarks across all directories:
\`\`\`bash
node ${mode === 'jitless' ? '--jitless' : ''} run_benchmarks.js
\`\`\`

---

`;

  const sectionDescriptions = {
    'basic': 'Microbenchmarks targeting general JS control flow, loops, and function calls.',
    'closures': 'Benchmarks testing closures creation and invocation performance.',
    'intl': 'ECMA-402 Internationalization benchmarks powered by ICU4X.',
    'json': 'JSON serialization benchmarks.',
    'properties': 'Object property retrieval and setting performance benchmarks.',
    'prototypes': 'Prototype chain lookup performance.',
    'strings': 'Common JS string operations.',
    'v8-benches': 'The V8 octene/benchmark suite containing complex programs. Both sample sizes are shown here.'
  };

  const sectionOrder = ['basic', 'closures', 'intl', 'json', 'properties', 'prototypes', 'strings', 'v8-benches'];

  sectionOrder.forEach((dirName, idx) => {
    if (!groups[dirName]) return;

    md += `## ${idx + 1}. ${dirName}\n`;
    md += `${sectionDescriptions[dirName] || ''}\n\n`;
    md += `**Run Command:**\n\`\`\`bash\n`;
    if (dirName === 'v8-benches') {
      md += `node ${mode === 'jitless' ? '--jitless' : ''} run_benchmarks.js -- ${dirName}\n`;
    } else {
      md += `node ${mode === 'jitless' ? '--jitless' : ''} run_benchmarks.js -- ${dirName}\n`;
    }
    md += `\`\`\`\n\n`;

    const dataList = reportData[dirName];

    if (dirName === 'v8-benches') {
      md += `| Benchmark | Time (10 Samples) | Time (100 Samples) | Comparison (100 vs 10) |\n`;
      md += `|---|---|---|---|\n`;
      dataList.forEach(item => {
        const t10 = item.time10;
        const t100 = item.time100;
        
        let compStr = 'N/A';
        if (!isNaN(t10) && !isNaN(t100) && t10 > 0) {
          if (t100 < t10) {
            const pct = ((t10 - t100) / t10) * 100;
            compStr = `${pct.toFixed(2)}% faster`;
          } else {
            const pct = ((t100 - t10) / t10) * 100;
            compStr = `${pct.toFixed(2)}% slower`;
          }
        }

        const t10Str = isNaN(t10) ? 'Error' : `${t10.toFixed(4)} ms`;
        const t100Str = isNaN(t100) ? 'Error' : `${t100.toFixed(4)} ms`;

        md += `| **\`${item.benchName}\`** | ${t10Str} | ${t100Str} | ${compStr} |\n`;
      });
    } else {
      md += `| Benchmark | Mean Execution Time | Sample Size |\n`;
      md += `|---|---|---|\n`;
      dataList.forEach(item => {
        const mean = item.meanTime;
        const meanStr = isNaN(mean) ? 'Error' : `${mean.toFixed(4)} ms`;
        md += `| **\`${item.benchName}\`** | ${meanStr} | 100 |\n`;
      });
    }

    md += `\n---\n\n`;
  });

  // Remove the trailing horizontal rule and newlines
  md = md.trim().replace(/\n---\s*$/, '');
  md += '\n';

  if (outputPath) {
    fs.writeFileSync(outputPath, md, 'utf8');
    console.log(`Report successfully written to ${outputPath}`);
  } else {
    console.log(md);
  }
}
