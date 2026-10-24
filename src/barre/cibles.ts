/**
 * Cibles `@` (spécification §4) : le logiciel qui ouvrira le résultat. Les noms sont ceux du Rust
 * (`cibles.rs`), sauf `bourse`, qui a ses propres commandes (`bourse.rs`), et `web` et `google`, qui ne
 * cherchent rien sur le poste : la saisie part dans Firefox, ou Chrome pour `google` (`chercher_sur_le_web`).
 */
export type NomCible = 'vsc' | 'terminal' | 'firefox' | 'notes' | 'fichier' | 'dossier' | 'bourse' | 'web' | 'google';

export interface DefinitionCible {
  nom: NomCible;
  /**
   * Un logiciel qui ouvre, ou un genre de résultat gardé : une cible de chaque sorte à la fois.
   * `seule` : une liste à part (`@bourse`, `@web`, `@google`), sans autre cible.
   */
  sorte: 'logiciel' | 'genre' | 'seule';
  libelle: string;
  /** Fin de la ligne d'aide : « Entrée pour … ». */
  action: string;
}

export const CIBLES: readonly DefinitionCible[] = [
  { nom: 'vsc', sorte: 'logiciel', libelle: 'VS Code', action: 'ouvrir dans VS Code' },
  { nom: 'terminal', sorte: 'logiciel', libelle: 'Terminal (Ghostty)', action: 'ouvrir un terminal dans le dossier' },
  { nom: 'firefox', sorte: 'logiciel', libelle: 'Firefox : pages, HTML, PDF', action: 'ouvrir dans Firefox' },
  { nom: 'notes', sorte: 'logiciel', libelle: 'Pages de Notes', action: 'ouvrir dans Notes' },
  { nom: 'fichier', sorte: 'genre', libelle: 'Fichiers seulement', action: 'ouvrir' },
  { nom: 'dossier', sorte: 'genre', libelle: 'Dossiers seulement', action: 'ouvrir le dossier' },
  { nom: 'bourse', sorte: 'seule', libelle: 'Bourse : cours et portefeuille', action: 'voir le cours' },
  { nom: 'web', sorte: 'seule', libelle: 'Recherche sur le Web, dans Firefox', action: 'chercher sur le Web' },
  { nom: 'google', sorte: 'seule', libelle: 'Recherche sur Google, dans Chrome', action: 'chercher sur Google' },
];

/** Autres noms d'une cible, avec ou sans `@` : `code` pour VS Code, `ghostty` pour le terminal, `recherche` pour le Web, `chrome` pour Google. */
const AUTRES_NOMS: Readonly<Record<string, NomCible>> = { code: 'vsc', ghostty: 'terminal', recherche: 'web', chrome: 'google' };

/** Cible qui porte ce nom complet (ou un autre de ses noms), sans tenir compte de la casse. */
function cibleNommee(nom: string | undefined): NomCible | null {
  const minuscule = nom?.toLowerCase();
  if (minuscule === undefined) {
    return null;
  }
  return CIBLES.find((cible) => cible.nom === minuscule)?.nom ?? AUTRES_NOMS[minuscule] ?? null;
}

/** « logiciel » ou « logiciels », avec ou sans `@`, en tête du champ et suivi d'une espace. */
const motLogiciels = /^\s*@?logiciels?\s+/iu;

/** Saisie qui ouvre la catégorie « Logiciels » : le reste du champ, sinon `null`. */
export function categorieTapee(texte: string): string | null {
  const mot = motLogiciels.exec(texte);
  return mot === null ? null : texte.slice(mot[0].length);
}

/** Mot en `@` en fin de saisie, en début de champ ou après une espace. */
const motEnArobase = /(^|\s)@(\S*)$/u;

/** Nom de cible reçu du système, ou `null` s'il n'en est pas une. */
export function nomDeCible(nom: string | null): NomCible | null {
  return CIBLES.find((cible) => cible.nom === nom)?.nom ?? null;
}

export function definition(nom: NomCible): DefinitionCible {
  return CIBLES.find((cible) => cible.nom === nom) ?? CIBLES[0]!;
}

/**
 * Cibles après avoir posé `nom` : il remplace celle de la même sorte, et passe en dernier. Une
 * cible `seule` remplace toutes les autres, et part quand une autre est posée.
 */
export function poser(cibles: readonly NomCible[], nom: NomCible): NomCible[] {
  const sorte = definition(nom).sorte;
  if (sorte === 'seule') {
    return [nom];
  }
  return [...cibles.filter((cible) => definition(cible).sorte !== sorte && definition(cible).sorte !== 'seule'), nom];
}

/** Action de la ligne d'aide : celle du logiciel posé, sinon celle du genre. */
export function actionDe(cibles: readonly NomCible[]): string {
  const logiciel = cibles.find((cible) => definition(cible).sorte === 'logiciel');
  return definition(logiciel ?? cibles[0] ?? 'fichier').action;
}

/** Saisie sans le mot en `@` de la fin. */
export function sansMotEnArobase(texte: string): string {
  return texte.replace(motEnArobase, '$1');
}

/** Cibles proposées pendant qu'un mot en `@` se tape (« @ » seul les propose toutes), sinon `null`. */
export function propositions(texte: string): DefinitionCible[] | null {
  const mot = motEnArobase.exec(texte);
  if (mot === null) {
    return null;
  }
  const debut = (mot[2] ?? '').toLowerCase();
  const trouvees = CIBLES.filter((cible) => cible.nom.startsWith(debut));
  return trouvees.length > 0 ? trouvees : null;
}

/** Mot en `@` suivi d'une espace, n'importe où dans la saisie. */
const motEnArobaseSuivi = /(^|\s)@(\S+)\s/u;

/** Premier mot du champ suivi d'une espace : une cible s'il en porte le nom, même sans `@`. */
const nomEnTete = /^\s*([a-z]+)\s+/iu;

/**
 * Cible tapée qui devient étiquette, le reste restant dans le champ : un nom complet en fin de
 * saisie (« rapport @vsc »), ou un début de nom suivi d'une espace (« @ter rapport »), qui prend
 * la première cible proposée, comme `Tab`. Sans `@`, seulement un nom complet en tête du champ suivi
 * d'une espace (« vsc rapport ») : un mot tapé seul reste une recherche.
 */
export function cibleTapee(texte: string): { cible: NomCible; reste: string } | null {
  // Sans `@` : un nom complet en tête du champ, suivi d'une espace (« vsc rapport »).
  const enTete = nomEnTete.exec(texte);
  const nue = cibleNommee(enTete?.[1]);
  if (nue !== null) {
    return { cible: nue, reste: texte.slice(enTete![0].length) };
  }
  const mot = motEnArobase.exec(texte);
  const complete = cibleNommee(mot?.[2]);
  if (complete !== null) {
    return { cible: complete, reste: sansMotEnArobase(texte) };
  }
  const suivi = motEnArobaseSuivi.exec(texte);
  const debut = suivi?.[2]?.toLowerCase();
  const abregee = debut === undefined ? undefined : CIBLES.find((c) => c.nom.startsWith(debut));
  return abregee === undefined ? null : { cible: abregee.nom, reste: texte.replace(motEnArobaseSuivi, '$1') };
}

/** Noms que `Tab` complète en tête du champ : les cibles, leurs autres noms, et la catégorie des logiciels. */
const NOMS_COMPLETES: readonly string[] = [...CIBLES.map((cible) => cible.nom), ...Object.keys(AUTRES_NOMS), 'logiciels'];

/** Un seul mot dans le champ, avec ou sans `@`, en cours de frappe. */
const motSeul = /^\s*(@?)([a-z]+)$/iu;

/**
 * Complétion proposée en grisé pendant que le premier mot se tape (« vs » → « vsc », « lo » →
 * « logiciels ») : deux lettres au moins sans `@`, une avec. `suite` est la partie grisée, `texte`
 * la saisie une fois complétée (nom suivi d'une espace : `changerTexte` en fait l'étiquette),
 * `filtre` ce que montre l'étiquette (« @vsc » pour `code`, « Logiciels »).
 * Les cibles déjà posées ne sont pas proposées.
 */
export function completion(texte: string, posees: readonly NomCible[]): { nom: string; suite: string; texte: string; filtre: string } | null {
  const mot = motSeul.exec(texte);
  const debut = mot?.[2]?.toLowerCase();
  if (debut === undefined || debut.length < (mot![1] === '@' ? 1 : 2)) {
    return null;
  }
  const nom = NOMS_COMPLETES.find((candidat) => {
    const cible = cibleNommee(candidat);
    return candidat.startsWith(debut) && (cible === null || !posees.includes(cible));
  });
  if (nom === undefined) {
    return null;
  }
  const cible = cibleNommee(nom);
  return { nom, suite: nom.slice(debut.length), texte: `${nom} `, filtre: cible === null ? 'Logiciels' : `@${cible}` };
}
