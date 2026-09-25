const fs = require('node:fs');
const path = require('node:path');

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || '')) throw new Error('Expected a stable X.Y.Z version');
const root = path.resolve(__dirname, '..', '..');
const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const lock = fs.readFileSync(path.join(root, 'Cargo.lock'), 'utf8');
const npm = require('../package.json');
const cargoVersion = cargo.match(/^version = "([^"]+)"/m)?.[1];
const lockVersion = lock.match(/\[\[package\]\]\s+name = "deordinal"\s+version = "([^"]+)"/)?.[1];
if (cargoVersion !== version || lockVersion !== version || npm.version !== version) {
  throw new Error(`Version mismatch: input=${version}, Cargo=${cargoVersion}, lock=${lockVersion}, npm=${npm.version}`);
}
console.log(`Release version ${version} matches Cargo.toml, Cargo.lock and npm/package.json`);
