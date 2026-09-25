const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const root = path.resolve(__dirname, '..', '..');
const manifest = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
const binary = path.resolve(process.argv[2] || '');
const result = spawnSync(binary, ['--version'], { encoding: 'utf8' });
if (!version || result.status !== 0 || result.stdout.trim() !== `deordinal ${version}`) {
  throw new Error(`Binary version check failed: ${result.error || result.stderr || result.stdout}`);
}
console.log(`Verified deordinal ${version}: ${binary}`);
