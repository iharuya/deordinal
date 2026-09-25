#!/usr/bin/env node

const { spawnSync } = require('node:child_process');
const path = require('node:path');
const os = require('node:os');

const targets = {
  'darwin-arm64': 'deordinal',
  'darwin-x64': 'deordinal',
  'linux-x64': 'deordinal',
  'win32-x64': 'deordinal.exe',
};

const target = `${process.platform}-${process.arch}`;
const executable = targets[target];
if (!executable) {
  console.error(`deordinal does not support ${target}`);
  process.exit(1);
}

const binary = path.join(__dirname, '..', 'vendor', target, executable);
const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`Unable to run deordinal for ${target}: ${result.error.message}`);
  process.exit(1);
}
process.exitCode = result.status ?? (128 + (os.constants.signals[result.signal] ?? 0));
