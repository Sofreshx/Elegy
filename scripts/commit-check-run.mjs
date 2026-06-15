#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const SCHEMA_VERSION = 3;
const DEFAULT_CONFIG_NAME = '.copilot/commit-checks.json';

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

function resolveConfig(configPath) {
  if (configPath) {
    if (!exists(configPath)) {
      die(`Config file not found: ${configPath}`);
    }
    const cfg = readJson(configPath);
    if (!cfg) die(`Invalid config file: ${configPath}`);
    return cfg;
  }

  const cwdDefault = path.join(process.cwd(), DEFAULT_CONFIG_NAME);
  if (exists(cwdDefault)) {
    const cfg = readJson(cwdDefault);
    if (cfg) return cfg;
  }

  const explicit = path.join(process.cwd(), '.copilot', 'commit-checks.json');
  if (exists(explicit)) {
    const cfg = readJson(explicit);
    if (cfg) return cfg;
  }

  die(`No commit-checks.json found. Run commit-check-setup.mjs first, or pass --config <path>`);
}

const SHIPPED_DEFAULTS = {
  threshold: 70,
  weights: {
    test: 0.40,
    coverage: 0.30,
    lint: 0.15,
    format: 0.15,
  },
  gates: ['typecheck'],
};

function runCommand(command, cwd, timeoutMs = 120000) {
  const result = spawnSync(command, [], {
    cwd,
    stdio: 'pipe',
    encoding: 'utf8',
    timeout: timeoutMs,
    shell: true,
  });

  const timedOut = result.error?.code === 'ETIMEDOUT';
  const killedBySignal = result.signal != null;
  const success = result.status === 0 && !killedBySignal && !timedOut;

  return {
    exitCode: result.status ?? (timedOut ? -1 : -2),
    success,
    timedOut,
    killedBySignal,
    stdout: (result.stdout || '').trim(),
    stderr: (result.stderr || '').trim(),
  };
}

function parseCoverageOutput(stdout, stderr) {
  const combined = stdout + '\n' + stderr;

  const lines = {};
  const lineMatch = combined.match(/(?:Lines?|lines?):\s*(\d+(?:\.\d+)?)%/i);
  if (lineMatch) lines.linePct = parseFloat(lineMatch[1]);

  const branchMatch = combined.match(/(?:Branches?|branches?):\s*(\d+(?:\.\d+)?)%/i);
  if (branchMatch) lines.branchPct = parseFloat(branchMatch[1]);

  const funcMatch = combined.match(/(?:Functions?|funcs?):\s*(\d+(?:\.\d+)?)%/i);
  if (funcMatch) lines.functionPct = parseFloat(funcMatch[1]);

  if (Object.keys(lines).length === 0) {
    const covMatch = combined.match(/(\d+(?:\.\d+)?)%/);
    if (covMatch) lines.linePct = parseFloat(covMatch[1]);
  }

  return lines;
}

function computeCoverageScore(metrics) {
  const linePct = metrics.linePct ?? 0;
  const branchPct = metrics.branchPct ?? 0;
  const functionPct = metrics.functionPct ?? 0;
  return Math.min(100, Math.max(0, linePct * 0.5 + branchPct * 0.3 + functionPct * 0.2));
}

function parseLaneSummary(name, result) {
  if (result.timedOut) return 'Timed out after 120s';
  if (result.killedBySignal) return `Killed by signal: ${result.signal}`;
  if (result.success) {
    if (name === 'test') {
      const passMatch = result.stdout.match(/(\d+)\s+(?:passed|tests?\s+passed)/i);
      const failMatch = result.stdout.match(/(\d+)\s+(?:failed|tests?\s+failed)/i);
      let summary = '';
      if (passMatch) summary += `${passMatch[1]} passed`;
      if (failMatch) summary += `, ${failMatch[1]} failed`;
      return summary || 'Passed';
    }
    if (name === 'lint') {
      const errMatch = result.stdout.match(/(\d+)\s+(?:errors?|problems?)/i);
      return errMatch ? `${errMatch[1]} issues found and fixed` : 'Clean';
    }
    if (name === 'format') return 'All files formatted';
    if (name === 'typecheck') return 'No type errors';
    return 'Passed';
  }
  const errLines = result.stderr.split('\n').filter(l => l).slice(0, 3).join('; ');
  return errLines || `Exit code ${result.exitCode}`;
}

export function runChecks(config, repoRoot, options = {}) {
  const { profile, selectedLanes, selectedGroup, skipLanes = new Map() } = options;
  const threshold = config.threshold ?? SHIPPED_DEFAULTS.threshold;
  const weights = config.weights ?? SHIPPED_DEFAULTS.weights;
  const gates = config.gates ?? SHIPPED_DEFAULTS.gates;
  const lanes = config.lanes ?? {};
  const groups = config.groups ?? {};

  // Filter enabled lane names
  let laneNames = Object.keys(lanes).filter(name => lanes[name].enabled !== false).sort();

  // Validate skip requests
  const overrideReasons = {};
  for (const [name, reason] of skipLanes) {
    const lane = lanes[name];
    if (!lane) die(`Unknown lane: ${name}`, 2);
    if (lane.skippable === false) die(`Lane "${name}" is not skippable`, 2);
    if (!reason) die(`Lane "${name}" requires a skip reason (use --reason)`, 2);
    overrideReasons[name] = reason || '';
  }

  // Apply profile filter
  if (profile) {
    laneNames = laneNames.filter(name => {
      const lane = lanes[name];
      return lane.defaultProfiles && lane.defaultProfiles.includes(profile);
    });
  }

  // Apply --lane filter
  if (selectedLanes) {
    laneNames = laneNames.filter(name => selectedLanes.includes(name));
  }

  // Apply --group filter
  if (selectedGroup) {
    laneNames = laneNames.filter(name => {
      const lane = lanes[name];
      return lane.group === selectedGroup;
    });
  }

  // Track lane count before skip removal for totalLanes reporting
  const laneNamesBeforeSkip = [...laneNames];

  // Remove skipped lanes
  laneNames = laneNames.filter(name => !skipLanes.has(name));

  const results = {};
  const logs = [];
  let anyGateFailed = false;
  let errorOutput = '';
  const requiredFailures = [];
  const skippedLanes = {};
  const skippedLaneKeys = new Set(skipLanes.keys());

  // Record skip events
  for (const [name, reason] of skipLanes) {
    skippedLanes[name] = reason;
    logs.push({
      timestamp: new Date().toISOString(),
      event: 'skip',
      lane: name,
      reason: reason || undefined,
    });
  }

  for (const name of laneNames) {
    const lane = lanes[name];
    const commands = lane.commands || [];
    const isGate = gates.includes(name);
    // Lane is blocking unless explicitly set to false (backward compatible with v1 configs)
    const isBlocking = lane.blocking !== false;

    logs.push({
      timestamp: new Date().toISOString(),
      event: 'lane_start',
      lane: name,
    });

    if (commands.length === 0) {
      results[name] = {
        status: 'SKIP',
        exitCode: 0,
        durationMs: 0,
        score: null,
        details: 'No commands configured',
      };
      logs.push({
        timestamp: new Date().toISOString(),
        event: 'lane_end',
        lane: name,
        status: 'SKIP',
        exitCode: 0,
        durationMs: 0,
      });
      continue;
    }

    let lanePassed = true;
    const laneResults = [];

    for (const cmd of commands) {
      const start = Date.now();
      const cmdCwd = lane.cwd ? path.resolve(repoRoot, lane.cwd) : repoRoot;
      const cmdTimeout = lane.timeoutMs ?? 120000;
      const result = runCommand(cmd, cmdCwd, cmdTimeout);
      const durationMs = Date.now() - start;

      laneResults.push({ command: cmd, ...result, durationMs });

      if (!result.success) {
        lanePassed = false;
        errorOutput += `[${name}] Command failed: ${cmd}\n  ${result.stderr.slice(0, 200)}\n`;
      }
    }

    let score = null;
    let coverageMetrics = {};
    let details = '';

    if (name === 'coverage' && lanePassed) {
      for (const r of laneResults) {
        const metrics = parseCoverageOutput(r.stdout, r.stderr);
        if (Object.keys(metrics).length > 0) {
          coverageMetrics = metrics;
        }
      }
      score = Math.round(computeCoverageScore(coverageMetrics));
      const parts = [];
      if (coverageMetrics.linePct != null) parts.push(`Lines: ${coverageMetrics.linePct}%`);
      if (coverageMetrics.branchPct != null) parts.push(`Branches: ${coverageMetrics.branchPct}%`);
      if (coverageMetrics.functionPct != null) parts.push(`Functions: ${coverageMetrics.functionPct}%`);
      details = parts.length > 0 ? parts.join(', ') : 'Coverage ran successfully';
    } else {
      const lastResult = laneResults[laneResults.length - 1];
      score = lanePassed ? 100 : 0;
      details = parseLaneSummary(name, lastResult);
    }

    if (isGate && isBlocking && !lanePassed) {
      anyGateFailed = true;
    }

    results[name] = {
      status: lanePassed ? 'PASS' : 'FAIL',
      exitCode: laneResults.every(r => r.success) ? 0 : 1,
      durationMs: laneResults.reduce((sum, r) => sum + (r.durationMs || 0), 0),
      score,
      commands: laneResults.map(r => ({
        command: r.command,
        exitCode: r.exitCode,
        success: r.success,
        durationMs: r.durationMs,
      })),
      details,
      group: lane.group ?? null,
      blocking: lane.blocking ?? true,
      ciWorkflow: lane.ciWorkflow ?? null,
      ciJob: lane.ciJob ?? null,
      ciRequired: lane.ciRequired ?? false,
      ...(name === 'coverage' && Object.keys(coverageMetrics).length > 0 ? { coverage: coverageMetrics } : {}),
    };

    logs.push({
      timestamp: new Date().toISOString(),
      event: 'lane_end',
      lane: name,
      status: lanePassed ? 'PASS' : 'FAIL',
      exitCode: laneResults.every(r => r.success) ? 0 : 1,
      durationMs: laneResults.reduce((sum, r) => sum + (r.durationMs || 0), 0),
    });

    if (lane.required !== false && !lanePassed) {
      requiredFailures.push(name);
    }
  }

  const scoredLanes = Object.keys(results).filter(name =>
    results[name]?.score != null && weights[name] != null && weights[name] > 0
  );

  let weightedSum = 0;
  let totalWeight = 0;
  const scoreBreakdown = [];

  for (const name of scoredLanes) {
    const weight = weights[name];
    const score = results[name].score;
    weightedSum += weight * score;
    totalWeight += weight;
    scoreBreakdown.push({ lane: name, weight, score, weighted: weight * score });
  }

  const compositeScore = totalWeight > 0 ? Math.round(weightedSum / totalWeight) : null;
  const passesThreshold = compositeScore != null && compositeScore >= threshold;
  const overallPass = passesThreshold && !anyGateFailed;

  const groupResults = {};
  for (const [name, result] of Object.entries(results)) {
    const group = result.group || '__ungrouped__';
    if (!groupResults[group]) {
      groupResults[group] = { passedLanes: [], failedLanes: [], allPassed: true };
    }
    if (result.status === 'PASS') {
      groupResults[group].passedLanes.push(name);
    } else {
      groupResults[group].failedLanes.push(name);
      groupResults[group].allPassed = false;
    }
  }

  const summary = {
    profile: profile || null,
    requiredFailures,
    skippedLanes,
    overrideReasons,
    logs,
    events: logs,
    schemaVersion: SCHEMA_VERSION,
    timestamp: new Date().toISOString(),
    repoRoot,
    threshold,
    compositeScore,
    overallPass,
    anyGateFailed,
    groups,
    groupResults,
    scoreBreakdown,
    totalLanes: laneNamesBeforeSkip.length,
    passedLanes: laneNames.filter(name => results[name]?.status === 'PASS').length,
    lanes: results,
  };

  if (errorOutput && !overallPass) {
    summary.errorOutput = errorOutput;
  }

  return summary;
}

function showHelp() {
  console.log(`
Usage: node scripts/commit-check-run.mjs [options]

Options:
  --config <path>     Path to commit-checks.json config file
  --repo <path>       Repo root directory (default: cwd)
  --json              Output results as JSON
  --profile <name>    Select profile (e.g. commit, ci-local, release)
  --lane <name>       Run only a specific named lane
  --group <name>      Run only lanes in a specific group
  --skip <name>       Skip a lane (can be repeated, e.g. --skip lint --skip test)
  --reason <text>     Reason for skip (applies to preceding --skip)
  --help, -h          Show this help message
  `);
}

function main() {
  const args = process.argv.slice(2);
  let configPath = null;
  let jsonOutput = false;
  let repoRoot = process.cwd();
  let profile = null;
  let selectedLanes = null;
  let selectedGroup = null;
  const skipLanes = new Map();
  let pendingReason = null;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--help' || args[i] === '-h') {
      showHelp();
      process.exit(0);
    } else if (args[i] === '--config' && i + 1 < args.length) {
      configPath = path.resolve(args[++i]);
    } else if (args[i] === '--repo' && i + 1 < args.length) {
      repoRoot = path.resolve(args[++i]);
    } else if (args[i] === '--json') {
      jsonOutput = true;
    } else if (args[i] === '--profile' && i + 1 < args.length) {
      profile = args[++i];
    } else if (args[i] === '--lane' && i + 1 < args.length) {
      selectedLanes = [args[++i]];
    } else if (args[i] === '--group' && i + 1 < args.length) {
      selectedGroup = args[++i];
    } else if (args[i] === '--skip' && i + 1 < args.length) {
      const laneName = args[++i];
      skipLanes.set(laneName, pendingReason || '');
      pendingReason = null;
    } else if (args[i] === '--reason' && i + 1 < args.length) {
      pendingReason = args[++i];
    } else if (!args[i].startsWith('--')) {
      repoRoot = path.resolve(args[i]);
    }
  }

  // If there's a trailing --reason with no subsequent --skip, ignore it
  if (pendingReason) {
    console.warn('[warn] --reason provided without a following --skip (ignored)');
  }

  const config = resolveConfig(configPath);
  const result = runChecks(config, repoRoot, {
    profile,
    selectedLanes,
    selectedGroup,
    skipLanes,
  });

  if (jsonOutput) {
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
  } else {
    console.log(`\n=== Commit Check Result ===`);
    console.log(`Score: ${result.compositeScore ?? 'N/A'} / 100  (threshold: ${result.threshold})`);
    console.log(`Status: ${result.overallPass ? 'PASS' : 'FAIL'}`);
    if (result.anyGateFailed) console.log('Gate failure: one or more gate lanes failed');
    if (result.profile) console.log(`Profile: ${result.profile}`);
    if (Object.keys(result.skippedLanes).length > 0) {
      console.log(`Skipped: ${Object.keys(result.skippedLanes).join(', ')}`);
    }
    if (result.requiredFailures.length > 0) {
      console.log(`Required failures: ${result.requiredFailures.join(', ')}`);
    }
    console.log(`\nLanes: ${result.passedLanes}/${result.totalLanes} passed\n`);
    for (const [name, lane] of Object.entries(result.lanes)) {
      const icon = lane.status === 'PASS' ? 'PASS' : lane.status === 'SKIP' ? 'SKIP' : 'FAIL';
      const scoreStr = lane.score != null ? ` (score: ${lane.score})` : '';
      console.log(`  ${icon}  ${name}${scoreStr} — ${lane.details}`);
    }
    console.log('');
  }

  const exitCode = result.overallPass ? 0 : 1;
  process.exit(exitCode);
}

if (process.argv[1]?.endsWith('commit-check-run.mjs')) {
  main();
}
