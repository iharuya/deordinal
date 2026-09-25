const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const artifacts = path.join(root, 'release-artifacts');
const vendor = path.join(root, 'npm', 'vendor');
const targets = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'win32-x64'];

const sources = targets.map((target) => {
  const name = target.startsWith('win32') ? 'deordinal.exe' : 'deordinal';
  const source = path.join(artifacts, `deordinal-${target}`, name);
  if (!fs.statSync(source, { throwIfNoEntry: false })?.isFile()) {
    throw new Error(`Missing artifact: ${source}`);
  }
  return { target, name, source };
});

fs.rmSync(vendor, { recursive: true, force: true });
for (const { target, name, source } of sources) {
  const destination = path.join(vendor, target, name);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
  if (!target.startsWith('win32')) fs.chmodSync(destination, 0o755);
}
console.log('Staged all four deordinal binaries in npm/vendor');
