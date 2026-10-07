const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || '')) throw new Error('Usage: node check-release-absent.cjs <version>');

const registries = {
  'crates.io': `https://crates.io/api/v1/crates/deordinal/${version}`,
  npm: `https://registry.npmjs.org/deordinal/${version}`,
};

async function isPublished(name, url) {
  const response = await fetch(url, {
    headers: { 'User-Agent': 'deordinal-release (https://github.com/iharuya/deordinal)' },
    signal: AbortSignal.timeout(10_000),
    cache: 'no-store',
  });
  if (response.status === 404) return false;
  if (!response.ok) throw new Error(`${name} returned HTTP ${response.status}`);
  return true;
}

async function run() {
  for (const [name, url] of Object.entries(registries)) {
    if (await isPublished(name, url)) throw new Error(`${name} already has deordinal ${version}; do not re-publish`);
  }
  console.log(`Version ${version} is not published on either registry`);
}

run().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
