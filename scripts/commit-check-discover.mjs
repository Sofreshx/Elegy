#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const SCHEMA_VERSION = 1;

function die(message, code = 1) {
  console.error(message);
  process.exit(code);
}

function exists(filePath) {
  return fs.existsSync(filePath);
}

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
  } catch {
    return null;
  }
}

function collectWorkspaceDeps(repoRoot, pkg) {
  const allDeps = { ...pkg.dependencies, ...pkg.devDependencies };
  const workspaces = pkg.workspaces || [];

  for (const ws of workspaces) {
    const wsRoot = path.join(repoRoot, ws.replace(/\/+$/, ''));
    const wsPkgPath = path.join(wsRoot, 'package.json');
    if (!exists(wsPkgPath)) continue;
    const wsPkg = readJson(wsPkgPath);
    if (!wsPkg) continue;
    Object.assign(allDeps, wsPkg.dependencies, wsPkg.devDependencies);
  }

  return allDeps;
}

function detectTypeScriptLanes(repoRoot) {
  const lanes = {};
  const pkgJsonPath = path.join(repoRoot, 'package.json');
  if (!exists(pkgJsonPath)) {
    return { lanes, found: false };
  }

  const pkg = readJson(pkgJsonPath);
  if (!pkg) {
    return { lanes, found: false };
  }

  const scripts = pkg.scripts || {};
  const deps = collectWorkspaceDeps(repoRoot, pkg);

  const hasTsConfig = exists(path.join(repoRoot, 'tsconfig.json')) ||
    exists(path.join(repoRoot, 'tsconfig.build.json'));

  const eslintConfigs = [
    '.eslintrc', '.eslintrc.json', '.eslintrc.js', '.eslintrc.yaml', '.eslintrc.yml',
    'eslint.config.js', 'eslint.config.mjs', 'eslint.config.cjs',
  ];
  const hasEslint = eslintConfigs.some(c => exists(path.join(repoRoot, c))) ||
    deps.eslint !== undefined;

  const prettierConfigs = [
    '.prettierrc', '.prettierrc.json', '.prettierrc.js', '.prettierrc.yaml', '.prettierrc.yml',
    '.prettierrc.toml', 'prettier.config.js',
  ];
  const hasPrettier = prettierConfigs.some(c => exists(path.join(repoRoot, c))) ||
    deps.prettier !== undefined;

  const hasVitest = deps.vitest !== undefined;
  const hasJest = deps.jest !== undefined;
  const hasTsc = deps.typescript !== undefined || hasTsConfig;

  if (hasVitest || hasJest) {
    const testScript = scripts.test || scripts['test:unit'] || null;
    const testFramework = hasVitest ? 'vitest' : 'jest';
    const defaultCmd = hasVitest ? 'npx vitest run' : 'npx jest';

    let commands = [];
    if (testScript && testScript !== 'echo "no tests"') {
      const scriptName = Object.entries(scripts).find(([,v]) => v === testScript)?.[0] || 'test';
      commands.push(`npm run ${scriptName}`);
    }
    if (commands.length === 0) {
      commands.push(defaultCmd);
    }

    lanes.test = { found: true, commands, framework: testFramework };

    const coverageScript = scripts['test:coverage'] || scripts.coverage || null;
    const hasCoverageConfig = (hasVitest &&
      (exists(path.join(repoRoot, 'vitest.config.ts')) ||
       exists(path.join(repoRoot, 'vitest.config.mts')) ||
       exists(path.join(repoRoot, 'vitest.config.js'))));
    const hasC8 = deps.c8 !== undefined || deps['@vitest/coverage-v8'] !== undefined;

    if (coverageScript || hasCoverageConfig || hasC8) {
      let coverageCommands = coverageScript
        ? [`npm run ${Object.entries(scripts).find(([,v]) => v === coverageScript)?.[0] || 'test:coverage'}`]
        : [hasVitest ? 'npx vitest run --coverage' : 'npx jest --coverage'];
      lanes.coverage = { found: true, commands: coverageCommands, tool: hasVitest ? 'vitest/istanbul' : 'jest/istanbul' };
    } else if (hasCoverageConfig) {
      lanes.coverage = { found: true, commands: ['npx vitest run --coverage'], tool: 'vitest/istanbul' };
    }
  }

  if (hasEslint) {
    const lintScript = scripts.lint || null;
    let commands = lintScript
      ? [`npm run ${Object.entries(scripts).find(([,v]) => v === lintScript)?.[0] || 'lint'}`]
      : ['npx eslint .'];
    lanes.lint = { found: true, commands, linter: 'eslint' };
  }

  if (hasPrettier) {
    const formatScript = scripts.format || scripts['format:check'] || null;
    let commands = formatScript
      ? [`npm run ${Object.entries(scripts).find(([,v]) => v === formatScript)?.[0] || 'format'}`]
      : ['npx prettier --check .'];
    lanes.format = { found: true, commands, formatter: 'prettier' };
  }

  if (hasTsc) {
    const typecheckScript = scripts.typecheck || scripts['type:check'] || scripts['tsc:check'] || null;
    let commands = typecheckScript
      ? [`npm run ${Object.entries(scripts).find(([,v]) => v === typecheckScript)?.[0] || 'typecheck'}`]
      : ['npx tsc --noEmit'];
    lanes.typecheck = { found: true, commands, tool: 'tsc' };
  }

  return { lanes, found: Object.keys(lanes).length > 0 };
}

function detectRustTool(toolName) {
  try {
    const r = spawnSync(`${toolName} --version`, [], {
      stdio: 'pipe',
      encoding: 'utf8',
      timeout: 5000,
      shell: true,
    });
    return r.status === 0;
  } catch {
    return false;
  }
}

function parseCargoToml(cargoTomlPath) {
  const content = fs.readFileSync(cargoTomlPath, 'utf8');
  const result = {};
  const memberMatch = content.match(/members\s*=\s*\[([^\]]+)\]/s);
  if (memberMatch) {
    result.members = memberMatch[1]
      .split(',')
      .map(m => m.trim().replace(/["'\s]/g, ''))
      .filter(Boolean);
  }
  return result;
}

function detectRustLanes(repoRoot) {
  const lanes = {};
  const cargoTomlPath = path.join(repoRoot, 'Cargo.toml');
  if (!exists(cargoTomlPath)) {
    return { lanes, found: false };
  }

  const cargoInfo = parseCargoToml(cargoTomlPath);

  lanes.test = { found: true, commands: ['cargo test'], framework: 'cargo-test' };
  lanes.typecheck = { found: true, commands: ['cargo check'], tool: 'cargo-check' };
  lanes.lint = { found: true, commands: ['cargo clippy -- -D warnings'], linter: 'clippy' };
  lanes.format = { found: true, commands: ['cargo fmt -- --check'], formatter: 'rustfmt' };

  const hasTarpaulin = detectRustTool('cargo-tarpaulin');
  const hasLlvmCov = detectRustTool('cargo-llvm-cov');

  if (hasLlvmCov || hasTarpaulin) {
    lanes.coverage = {
      found: true,
      commands: [hasLlvmCov ? 'cargo llvm-cov' : 'cargo tarpaulin'],
      tool: hasLlvmCov ? 'cargo-llvm-cov' : 'cargo-tarpaulin',
    };
  } else {
    lanes.coverage = {
      found: false,
      commands: [],
      tool: null,
      note: 'Rust coverage requires cargo-llvm-cov or cargo-tarpaulin. Install with: cargo install cargo-llvm-cov',
    };
  }

  return { lanes, found: true };
}

function detectExistingConfig(repoRoot) {
  const configPath = path.join(repoRoot, '.copilot', 'commit-checks.json');
  if (exists(configPath)) {
    const config = readJson(configPath);
    if (config && config.schemaVersion === SCHEMA_VERSION) {
      return { exists: true, valid: true, path: configPath, config };
    }
    return { exists: true, valid: false, path: configPath, config: null };
  }
  return { exists: false, valid: false, path: configPath, config: null };
}

export function discover(repoRoot) {
  if (!exists(repoRoot)) {
    throw new Error(`Repo root not found: ${repoRoot}`);
  }

  const tsLanes = detectTypeScriptLanes(repoRoot);
  const rustLanes = detectRustLanes(repoRoot);
  const existingConfig = detectExistingConfig(repoRoot);

  const allLanes = {};

  for (const [name, lane] of Object.entries(tsLanes.lanes)) {
    allLanes[name] = { ...lane, source: 'typescript' };
  }

  for (const [name, lane] of Object.entries(rustLanes.lanes)) {
    if (allLanes[name]) {
      if (lane.found) {
        allLanes[name].commands = [...allLanes[name].commands, ...lane.commands];
        allLanes[name].source = 'multi';
      }
    } else {
      allLanes[name] = { ...lane, source: 'rust' };
    }
  }

  const languages = [];
  if (tsLanes.found) languages.push('typescript');
  if (rustLanes.found) languages.push('rust');

  return {
    schemaVersion: SCHEMA_VERSION,
    repoRoot,
    languages,
    configExists: existingConfig.exists,
    configValid: existingConfig.valid,
    configPath: existingConfig.path,
    lanes: allLanes,
    summary: {
      totalLanes: Object.keys(allLanes).length,
      foundLanes: Object.values(allLanes).filter(l => l.found).length,
      languages,
    },
  };
}

function main() {
  const repoRoot = process.argv[2] || process.cwd();
  try {
    const result = discover(repoRoot);
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
  } catch (err) {
    die(`Discovery failed: ${err.message}`);
  }
}

if (process.argv[1]?.endsWith('commit-check-discover.mjs')) {
  main();
}
