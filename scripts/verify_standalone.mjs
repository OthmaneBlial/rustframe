#!/usr/bin/env node
// Local package integration, deliberately distinct from registry-only release proof.
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { performance } from 'node:perf_hooks';

const args = process.argv.slice(2);
const option = name => {
  const index = args.indexOf(name);
  if (index < 0 || !args[index + 1] || args[index + 1].startsWith('--')) {
    throw new Error(`Required: ${name} <path>`);
  }
  return resolve(args[index + 1]);
};
const cli = option('--cli');
const api = option('--api-tarball');
const output = option('--output');
mkdirSync(output, { recursive: true });
const root = mkdtempSync(join(tmpdir(), 'rustframe-standalone-'));
const receipt = { kind: 'local-package-integration', registryOnly: false, root,
  host: { platform: process.platform, architecture: process.arch, node: process.version },
  startedAt: new Date().toISOString(), success: false, projects: [] };
const save = () => writeFileSync(join(output, 'standalone.json'), JSON.stringify(receipt, null, 2) + '\n');
function run(command, argv, cwd, label, steps) {
  const start = performance.now();
  const result = spawnSync(command, argv, { cwd, encoding: 'utf8', timeout: 300_000,
    shell: process.platform === 'win32', maxBuffer: 16 * 1024 * 1024 });
  const log = `${label}.log`;
  writeFileSync(join(output, log), `${result.stdout || ''}${result.stderr || ''}${result.error || ''}`);
  steps.push({ command: [command, ...argv], durationMs: Math.round(performance.now() - start), exitCode: result.status, log });
  save();
  if (result.status !== 0 || result.error) throw new Error(`${label} failed; see ${join(output, log)}`);
  return result.stdout.trim();
}
try {
  receipt.cliVersion = run(cli, ['--version'], root, 'version', []);
  for (const template of ['vanilla-ts', 'react-ts', 'vanilla-js', 'vue-ts', 'svelte-ts']) {
    const project = { template, steps: [], success: false };
    receipt.projects.push(project);
    const name = `standalone-${template}`;
    const directory = join(root, name);
    run(cli, ['new', name, '--template', template, '--package-manager', 'npm'], root, `${template}-new`, project.steps);
    const manifest = JSON.parse(readFileSync(join(directory, 'package.json')));
    project.generatedApiRequirement = manifest.dependencies?.['rustframe-api'];
    if (project.generatedApiRequirement !== `=${receipt.cliVersion.split(' ').at(-1)}`) {
      throw new Error(`${template}: generated API version does not match CLI`);
    }
    // Only after recording the public dependency contract, substitute the exact local package.
    run('npm', ['install', api], directory, `${template}-install`, project.steps);
    run('npm', ['run', 'build'], directory, `${template}-frontend-build`, project.steps);
    run(cli, ['validate'], directory, `${template}-validate`, project.steps);
    project.success = true;
    save();
  }
  receipt.success = true;
} finally {
  receipt.finishedAt = new Date().toISOString();
  save();
  console.log(`Receipt: ${join(output, 'standalone.json')}`);
  console.log(`Isolated projects retained at: ${root}`);
}
