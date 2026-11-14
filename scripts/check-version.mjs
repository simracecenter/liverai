// Fail unless the release tag, the Cargo workspace version and package.json
// agree. Usage: node scripts/check-version.mjs v0.0.1
// With no argument it only checks that Cargo.toml and package.json agree.
import { readFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const pkg = JSON.parse(readFileSync(new URL('package.json', root), 'utf8')).version;

const cargoToml = readFileSync(new URL('Cargo.toml', root), 'utf8');
const section = /^\[workspace\.package\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(cargoToml)?.[1];
const cargo = section ? /^version\s*=\s*"([^"]+)"/m.exec(section)?.[1] : undefined;

const tag = process.argv[2];
const errors = [];
if (!cargo) errors.push('No version found in [workspace.package] of Cargo.toml.');
if (cargo && cargo !== pkg) {
  errors.push(`Cargo.toml [workspace.package] is ${cargo} but package.json is ${pkg}.`);
}
if (tag !== undefined) {
  if (!/^v\d+\.\d+\.\d+$/.test(tag)) {
    errors.push(`Tag "${tag}" is not of the form v<major>.<minor>.<patch>.`);
  } else if (tag.slice(1) !== cargo || tag.slice(1) !== pkg) {
    errors.push(`Tag ${tag} does not match Cargo.toml (${cargo}) and package.json (${pkg}).`);
  }
}

if (errors.length > 0) {
  for (const error of errors) console.error(`version check: ${error}`);
  process.exit(1);
}
console.log(`version check: ${tag ?? '(no tag)'} · Cargo.toml ${cargo} · package.json ${pkg}`);
