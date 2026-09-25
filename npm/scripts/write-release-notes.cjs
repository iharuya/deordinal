const fs = require('node:fs');

const version = process.env.VERSION;
if (!/^\d+\.\d+\.\d+$/.test(version || '')) throw new Error('Invalid release version');
const event = JSON.parse(fs.readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
const notes = event.inputs?.notes?.trim();
if (!notes) throw new Error('Release notes must not be empty');
fs.writeFileSync('release-notes.md', `${notes}\n\n- [crates.io](https://crates.io/crates/deordinal/${version})\n- [npm](https://www.npmjs.com/package/deordinal/v/${version})\n`);
