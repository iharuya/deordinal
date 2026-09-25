const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const root = path.resolve(__dirname, '..', '..');
const pkg = require('../package.json');
const manifest = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const version = manifest.match(/^version = "([^"]+)"/m)?.[1];
if (pkg.version !== version) throw new Error('npm and Cargo versions differ');
if (fs.readFileSync(path.join(root, 'LICENSE'), 'utf8') !==
    fs.readFileSync(path.join(root, 'npm', 'LICENSE'), 'utf8')) {
  throw new Error('npm/LICENSE differs from root LICENSE');
}

for (const target of ['darwin-arm64', 'darwin-x64', 'linux-x64', 'win32-x64']) {
  const name = target.startsWith('win32') ? 'deordinal.exe' : 'deordinal';
  const binary = path.join(root, 'npm', 'vendor', target, name);
  if (!fs.statSync(binary, { throwIfNoEntry: false })?.isFile()) {
    throw new Error(`Missing binary: ${binary}`);
  }
}

const result = spawnSync(process.execPath, [path.join(root, 'npm', 'bin', 'deordinal.cjs'), '--version'], {
  encoding: 'utf8',
});
if (result.status !== 0 || result.stdout.trim() !== `deordinal ${version}`) {
  throw new Error(`Host binary version check failed: ${result.stderr || result.stdout}`);
}
console.log(`Verified deordinal ${version} binaries and host launcher`);
