const fs = require('node:fs');
const crypto = require('node:crypto');

const [mode, version, tarball] = process.argv.slice(2);
if (!['absent', 'visible'].includes(mode) || !/^\d+\.\d+\.\d+$/.test(version || '')) {
  throw new Error('Usage: node release-registry.cjs <absent|visible> <version> [tarball]');
}
if (mode === 'visible' && !tarball) throw new Error('Visible check requires the tested tarball');

const integrity = tarball && `sha512-${crypto.createHash('sha512').update(fs.readFileSync(tarball)).digest('base64')}`;
class RegistryMismatch extends Error {}

const registries = {
  'crates.io': `https://crates.io/api/v1/crates/deordinal/${version}`,
  npm: `https://registry.npmjs.org/deordinal/${version}`,
};

async function check(name, url) {
  const response = await fetch(url, {
    headers: { 'User-Agent': 'deordinal-release (https://github.com/iharuya/deordinal)' },
    signal: AbortSignal.timeout(10_000),
    cache: 'no-store',
  });
  if (response.status === 404) return false;
  if (!response.ok) throw new Error(`${name} returned HTTP ${response.status}`);
  const data = await response.json();
  const actual = name === 'npm' ? data.version : data.version?.num;
  if (actual !== version) throw new RegistryMismatch(`${name} returned unexpected version ${actual}`);
  if (name === 'npm' && integrity && data.dist?.integrity !== integrity) {
    throw new RegistryMismatch('Published npm tarball differs from tested tarball');
  }
  return true;
}

async function run() {
  if (mode === 'absent') {
    for (const [name, url] of Object.entries(registries)) {
      if (await check(name, url)) throw new Error(`${name} already has deordinal ${version}; do not re-publish`);
    }
    console.log(`Version ${version} is not published on either registry`);
    return;
  }

  const deadline = Date.now() + 5 * 60_000;
  while (true) {
    try {
      const results = await Promise.all(Object.entries(registries).map(async ([name, url]) => [name, await check(name, url)]));
      if (results.every(([, found]) => found)) {
        console.log(`deordinal ${version} is visible on both registries; npm integrity matches`);
        return;
      }
      console.log(`Waiting for registry visibility: ${results.filter(([, found]) => !found).map(([name]) => name).join(', ')}`);
    } catch (error) {
      if (error instanceof RegistryMismatch) throw error;
      console.error(`Read-only registry check failed: ${error.message}`);
    }
    if (Date.now() >= deadline) throw new Error('Registry visibility timed out; inspect both registries before taking action');
    await new Promise((resolve) => setTimeout(resolve, 15_000));
  }
}

run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
