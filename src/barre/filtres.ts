/**
 * Filtres (spécification §4) : un seul concept pour ce qui était les cibles `@` et les catégories.
 * Un filtre de genre ne garde qu'un type de résultat (une section du Rust, `cibles::SECTIONS`), un
 * filtre de logiciel ouvre le résultat dans ce logiciel (une cible du Rust, `cibles.rs`), un filtre
 * seul a sa propre liste (`bourse.rs`) ou cherche sur le Web (`chercher_sur_le_web`).
 */
import type { Categorie } from './sections';

export type NomFiltre =
  | 'recents'
  | 'logiciels'
  | 'applications'
  | 'notes'
  | 'dossiers'
  | 'fichiers'
  | 'marque-pages'
  | 'historique'
  | 'vsc'
  | 'terminal'
  | 'firefox'
  | 'web'
  | 'google'
  | 'bourse';

export interface DefinitionFiltre {
  nom: NomFiltre;
  /** Un genre et un logiciel peuvent être posés ensemble ; un filtre `seule` remplace tous les autres. */
  sorte: 'genre' | 'logiciel' | 'seule';
  /** Nom montré dans l'étiquette et dans la ligne du filtre. */
  libelle: string;
  /** Ce qu'il fait, à droite de sa ligne. */
  description: string;
  /** Autres noms qui le désignent quand on les tape. */
  autresNoms: readonly string[];
  /** Section du Rust gardée, pour un genre. */
  section?: Categorie;
  /** Cible du Rust : le logiciel, ou le genre qui en a une (`dossier`, `fichier`). */
  cible?: string;
}

export const FILTRES: readonly DefinitionFiltre[] = [
  { nom: 'recents', sorte: 'genre', libelle: 'Récents', description: 'Ouverts dernièrement', autresNoms: ['recent'], section: 'Récents' },
  {
    nom: 'logiciels',
    sorte: 'genre',
    libelle: 'Logiciels',
    description: 'Tes logiciels',
    autresNoms: ['logiciel'],
    section: 'Logiciels',
  },
  {
    nom: 'applications',
    sorte: 'genre',
    libelle: 'Applications',
    description: 'Applications du poste',
    autresNoms: ['application', 'apps'],
    section: 'Applications',
  },
  { nom: 'notes', sorte: 'genre', libelle: 'Notes', description: 'Pages de Notes', autresNoms: ['note'], section: 'Notes' },
  {
    nom: 'dossiers',
    sorte: 'genre',
    libelle: 'Dossiers',
    description: 'Dossiers seulement',
    autresNoms: ['dossier'],
    section: 'Dossiers',
    cible: 'dossier',
  },
  {
    nom: 'fichiers',
    sorte: 'genre',
    libelle: 'Fichiers',
    description: 'Fichiers seulement',
    autresNoms: ['fichier'],
    section: 'Fichiers',
    cible: 'fichier',
  },
  {
    nom: 'marque-pages',
    sorte: 'genre',
    libelle: 'Marque-pages',
    description: 'Marque-pages de Firefox',
    autresNoms: ['marque-page', 'favoris'],
    section: 'Marque-pages',
  },
  {
    nom: 'historique',
    sorte: 'genre',
    libelle: 'Historique',
    description: 'Historique de Firefox',
    autresNoms: [],
    section: 'Historique',
  },
  {
    nom: 'vsc',
    sorte: 'logiciel',
    libelle: 'VS Code',
    description: 'Ouvrir dans VS Code',
    autresNoms: ['code', 'vscode'],
    cible: 'vsc',
  },
  {
    nom: 'terminal',
    sorte: 'logiciel',
    libelle: 'Terminal',
    description: 'Terminal dans le dossier',
    autresNoms: ['ghostty'],
    cible: 'terminal',
  },
  {
    nom: 'firefox',
    sorte: 'logiciel',
    libelle: 'Firefox',
    description: 'Pages, HTML et PDF dans Firefox',
    autresNoms: [],
    cible: 'firefox',
  },
  { nom: 'web', sorte: 'seule', libelle: 'Web', description: 'Chercher sur le Web, dans Firefox', autresNoms: ['recherche'] },
  { nom: 'google', sorte: 'seule', libelle: 'Google', description: 'Chercher sur Google, dans Chrome', autresNoms: ['chrome'] },
  { nom: 'bourse', sorte: 'seule', libelle: 'Bourse', description: 'Cours et portefeuille', autresNoms: [] },
];

/** Minuscules sans accents : « Récents » se tape « recents ». */
function normaliser(texte: string): string {
  return texte.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
}

export function definition(nom: NomFiltre): DefinitionFiltre {
  return FILTRES.find((filtre) => filtre.nom === nom) ?? FILTRES[0]!;
}

/** Noms qui désignent le filtre : le sien, ses autres noms et son libellé. */
function noms(filtre: DefinitionFiltre): string[] {
  return [filtre.nom, ...filtre.autresNoms, normaliser(filtre.libelle)];
}

/** Filtre qui porte ce nom complet (ou un autre de ses noms), sans tenir compte de la casse ni des accents. */
export function filtreNomme(nom: string | undefined): NomFiltre | null {
  if (nom === undefined || nom === '') {
    return null;
  }
  const cherche = normaliser(nom);
  return FILTRES.find((filtre) => noms(filtre).includes(cherche))?.nom ?? null;
}

/** Filtres dont un nom commence par `debut`, dans l'ordre du tableau, ceux déjà posés sautés. */
function commencantPar(debut: string, poses: readonly NomFiltre[]): DefinitionFiltre[] {
  const cherche = normaliser(debut);
  return FILTRES.filter((filtre) => !poses.includes(filtre.nom) && noms(filtre).some((nom) => nom.startsWith(cherche)));
}

/** Filtre demandé par le système (Ctrl+K depuis Notes, `--cible=`), donné sous son nom du Rust. */
export function filtreDuSysteme(nom: string | null): NomFiltre | null {
  return FILTRES.find((filtre) => filtre.cible === nom || filtre.nom === nom)?.nom ?? null;
}

/** Filtre de genre qui garde cette section (« Plus de résultats : Dossiers »). */
export function filtreDeSection(section: string): NomFiltre | null {
  return FILTRES.find((filtre) => filtre.section === section)?.nom ?? null;
}

/**
 * Filtres après avoir posé `nom` : il remplace celui de la même sorte, et passe en dernier. Un
 * filtre `seule` remplace tous les autres, et part quand un autre est posé.
 */
export function poser(poses: readonly NomFiltre[], nom: NomFiltre): NomFiltre[] {
  const sorte = definition(nom).sorte;
  if (sorte === 'seule') {
    return [nom];
  }
  return [...poses.filter((pose) => definition(pose).sorte !== sorte && definition(pose).sorte !== 'seule'), nom];
}

/** Ce que reçoit le Rust : les cibles (logiciel, `dossier` ou `fichier`) et la section du genre posé. */
export function pourLeSysteme(poses: readonly NomFiltre[]): { cibles: string[]; section: Categorie | null } {
  const definitions = poses.map(definition);
  return {
    cibles: definitions.flatMap((filtre) => (filtre.cible === undefined ? [] : [filtre.cible])),
    section: definitions.find((filtre) => filtre.section !== undefined)?.section ?? null,
  };
}

/** Mot en `@` en fin de saisie, en début de champ ou après une espace. */
const motEnArobase = /(^|\s)@(\S*)$/u;

/** Saisie sans le mot en `@` de la fin. */
export function sansMotEnArobase(texte: string): string {
  return texte.replace(motEnArobase, '$1');
}

/** Filtres proposés pendant qu'un mot en `@` se tape (« @ » seul les propose tous), sinon `null`. */
export function propositions(texte: string, poses: readonly NomFiltre[] = []): DefinitionFiltre[] | null {
  const mot = motEnArobase.exec(texte);
  if (mot === null) {
    return null;
  }
  const trouves = commencantPar(mot[2] ?? '', poses);
  return trouves.length > 0 ? trouves : null;
}

/** Un seul mot dans le champ, sans `@`, en cours de frappe. */
const motSeul = /^\s*([\p{L}-]+)$/u;

/**
 * Filtres proposés en tête de la liste pendant que le seul mot du champ commence un de leurs noms
 * (« vs » : VS Code, « dos » : Dossiers), deux lettres au moins. `Entrée` ou `Tab` en pose un.
 */
export function filtresProposes(texte: string, poses: readonly NomFiltre[]): NomFiltre[] {
  const mot = motSeul.exec(texte)?.[1];
  if (mot === undefined || mot.length < 2) {
    return [];
  }
  return commencantPar(mot, poses).map((filtre) => filtre.nom);
}

/** Mot en `@` suivi d'une espace, n'importe où dans la saisie. */
const motEnArobaseSuivi = /(^|\s)@(\S+)\s/u;

/** Premier mot du champ suivi d'une espace : un filtre s'il en porte un nom, même sans `@`. */
const nomEnTete = /^\s*([\p{L}-]+)\s+/u;

/**
 * Filtre tapé qui devient étiquette, le reste restant dans le champ : un nom complet en tête du
 * champ suivi d'une espace (« vsc rapport »), un nom complet en `@` en fin de saisie
 * (« rapport @vsc »), ou un début de nom en `@` suivi d'une espace (« @ter rapport »), qui prend le
 * premier filtre proposé.
 */
export function filtreTape(texte: string): { filtre: NomFiltre; reste: string } | null {
  const enTete = nomEnTete.exec(texte);
  const nu = filtreNomme(enTete?.[1]);
  if (nu !== null) {
    return { filtre: nu, reste: texte.slice(enTete![0].length) };
  }
  const mot = motEnArobase.exec(texte);
  const complet = filtreNomme(mot?.[2]);
  if (complet !== null) {
    return { filtre: complet, reste: sansMotEnArobase(texte) };
  }
  const suivi = motEnArobaseSuivi.exec(texte);
  const abrege = suivi?.[2] === undefined ? undefined : commencantPar(suivi[2], [])[0];
  return abrege === undefined ? null : { filtre: abrege.nom, reste: texte.replace(motEnArobaseSuivi, '$1') };
}
