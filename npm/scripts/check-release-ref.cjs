const { spawnSync } = require('node:child_process');

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version || '')) throw new Error('Expected a stable X.Y.Z version');

function remote(ref) {
  return spawnSync('git', ['ls-remote', '--exit-code', 'origin', ref], { encoding: 'utf8' });
}

const main = remote('refs/heads/main');
if (main.status !== 0) throw new Error(`Unable to check origin/main: ${main.stderr}`);
if (main.stdout.split('\t')[0] !== process.env.GITHUB_SHA) {
  throw new Error('main advanced since this workflow started; review and start a new run');
}
const tag = remote(`refs/tags/v${version}`);
if (tag.status === 0) throw new Error(`Tag v${version} already exists; do not overwrite it`);
if (tag.status !== 2) throw new Error(`Unable to check tag v${version}: ${tag.stderr}`);
console.log(`origin/main is ${process.env.GITHUB_SHA}; v${version} is not tagged`);
