#!/usr/bin/env node
// Vérifie que la carte du code (.claude/CARTE.md et .claude/carte/*.md) et le dépôt disent la même chose :
// tout fichier source figure dans la carte, et toute entrée cochée existe.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const racine = join(dirname(fileURLToPath(import.meta.url)), '..');
const dossiersSource = ['src', 'src-tauri/src', 'scripts'];
const extensionsSource = /\.(ts|tsx|js|mjs|rs|css|html|sh)$/;
const estUnTest = (chemin) => /\.test\.tsx?$/.test(chemin);

function parcourir(chemin) {
  if (!existsSync(chemin)) {
    return [];
  }
  if (statSync(chemin).isFile()) {
    return [chemin];
  }
  return readdirSync(chemin, { withFileTypes: true }).flatMap((entree) => {
    if (entree.name === 'node_modules' || entree.name.startsWith('.')) {
      return [];
    }
    return parcourir(join(chemin, entree.name));
  });
}

/** Entrées d'une carte : lignes « - [x] `chemin` ... » ou « - [ ] `chemin` ... », plusieurs chemins possibles avant « : ». */
function lireCarte(fichier) {
  const entrees = [];
  for (const ligne of readFileSync(fichier, 'utf8').split('\n')) {
    const debut = /^- \[( |x)\] /.exec(ligne);
    if (debut === null) {
      continue;
    }
    const description = ligne.indexOf(' : ');
    const zone = description === -1 ? ligne : ligne.slice(0, description);
    for (const [, chemin] of zone.matchAll(/`([^`]+)`/g)) {
      entrees.push({ chemin, existe: debut[1] === 'x' });
    }
  }
  return entrees;
}

const dossierCartes = join(racine, '.claude/carte');
const cartes = [join(racine, '.claude/CARTE.md')]
  .concat(existsSync(dossierCartes) ? readdirSync(dossierCartes).filter((nom) => nom.endsWith('.md')).map((nom) => join(dossierCartes, nom)) : [])
  .filter(existsSync);
if (cartes.length === 0) {
  // .claude/ n'est pas versionné : sans carte (clone neuf), il n'y a rien à vérifier.
  console.log('Pas de carte du code, rien à vérifier.');
  process.exit(0);
}

const entrees = cartes.flatMap(lireCarte);
const problemes = [];

for (const entree of entrees.filter((e) => e.existe && !e.chemin.includes('<') && !e.chemin.includes('*'))) {
  if (!existsSync(join(racine, entree.chemin))) {
    problemes.push(`« ${entree.chemin} » est coché mais n'existe pas`);
  }
}

const coches = entrees.filter((e) => e.existe).map((e) => e.chemin);
for (const dossier of dossiersSource) {
  for (const fichier of parcourir(join(racine, dossier))) {
    const chemin = relative(racine, fichier);
    if (!extensionsSource.test(chemin) || estUnTest(chemin)) {
      continue;
    }
    const couvert = coches.some((entree) => entree === chemin || (entree.endsWith('/') && chemin.startsWith(entree)));
    if (!couvert) {
      problemes.push(`« ${chemin} » existe mais n'est pas coché dans la carte`);
    }
  }
}

if (problemes.length > 0) {
  console.error(`Carte du code désynchronisée (${problemes.length}) :`);
  for (const probleme of problemes) {
    console.error(`  - ${probleme}`);
  }
  process.exit(1);
}
console.log('Carte du code à jour.');
