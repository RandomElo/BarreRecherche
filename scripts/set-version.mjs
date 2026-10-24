#!/usr/bin/env node
// Change le numéro de version partout d'un coup : package.json, Cargo.toml et les fichiers de verrouillage.
// tauri.conf.json lit la version dans package.json. Usage : npm run version:set -- 0.1.0
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const racine = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = process.argv[2];
if (version === undefined || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('Version attendue sous la forme 0.1.0 (npm run version:set -- 0.1.0).');
  process.exit(1);
}

/** Remplace la première occurrence du motif dans le fichier, ou échoue s'il n'y est pas. */
function remplacer(fichier, motif) {
  const chemin = join(racine, fichier);
  const contenu = readFileSync(chemin, 'utf8');
  if (!motif.test(contenu)) {
    console.error(`Champ de version introuvable dans ${fichier}.`);
    process.exit(1);
  }
  writeFileSync(chemin, contenu.replace(motif, `$1${version}$2`));
}

remplacer('package.json', /^(\s*"version":\s*")[^"]+(")/m);
remplacer('src-tauri/Cargo.toml', /^(version = ")[^"]+(")/m);
if (existsSync(join(racine, 'src-tauri/Cargo.lock'))) {
  remplacer('src-tauri/Cargo.lock', /(name = "barre-recherche"\nversion = ")[^"]+(")/);
}
execFileSync('npm', ['install', '--package-lock-only', '--ignore-scripts'], { cwd: racine, stdio: 'ignore' });
console.log(`Version ${version} appliquée.`);
