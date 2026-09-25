const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

const tarball = path.resolve(process.argv[2] || '');
const version = require('../package.json').version;
const contents = fs.readFileSync(tarball);
const hash = crypto.createHash('sha256').update(contents).digest('hex');
const unpacked = fs.mkdtempSync(path.join(os.tmpdir(), 'deordinal-pack-'));

try {
  const extract = spawnSync('tar', ['-xzf', tarball, '-C', unpacked], { encoding: 'utf8' });
  if (extract.status !== 0) throw new Error(extract.error || extract.stderr);

  const packed = path.join(unpacked, 'package');
  if (JSON.parse(fs.readFileSync(path.join(packed, 'package.json'), 'utf8')).version !== version) {
    throw new Error('Tarball version does not match npm/package.json');
  }
  for (const name of ['LICENSE', 'README.md', 'bin/deordinal.cjs']) {
    if (!fs.statSync(path.join(packed, name), { throwIfNoEntry: false })?.isFile()) {
      throw new Error(`Tarball is missing ${name}`);
    }
  }
  for (const target of ['darwin-arm64', 'darwin-x64', 'linux-x64', 'win32-x64']) {
    const name = target.startsWith('win32') ? 'deordinal.exe' : 'deordinal';
    const file = path.join(packed, 'vendor', target, name);
    const metadata = fs.statSync(file, { throwIfNoEntry: false });
    if (!metadata?.isFile()) throw new Error(`Tarball is missing ${target}/${name}`);
    if (!target.startsWith('win32') && (metadata.mode & 0o111) !== 0o111) {
      throw new Error(`Tarball lost executable permissions for ${target}/${name}`);
    }
  }
  const result = spawnSync(process.execPath, [path.join(packed, 'bin', 'deordinal.cjs'), '--version'], {
    encoding: 'utf8',
  });
  if (result.status !== 0 || result.stdout.trim() !== `deordinal ${version}`) {
    throw new Error(`Extracted launcher failed: ${result.error || result.stderr || result.stdout}`);
  }
  console.log(`Verified deordinal ${version} tarball; SHA-256 ${hash}`);
  if (process.env.GITHUB_STEP_SUMMARY) {
    fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `npm tarball SHA-256: \`${hash}\`\n\n`);
  }
} finally {
  fs.rmSync(unpacked, { recursive: true, force: true });
}
