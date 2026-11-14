import type { Resultat } from '../lib/systeme';
import { FILTRES, type NomFiltre, filtreDeSection } from './filtres';

/** Sections du Rust (`cibles::SECTIONS`), gardées chacune par un filtre de genre. */
export const CATEGORIES = ['Récents', 'Logiciels', 'Applications', 'Notes', 'Dossiers', 'Fichiers', 'Marque-pages', 'Historique'] as const;
export type Categorie = (typeof CATEGORIES)[number];

/** Ordre des sections dans la liste, toujours le même (spécification §6) ; une section inconnue vient après. */
const ORDRE_DES_SECTIONS: readonly string[] = ['Logiciels', 'Applications', 'Notes', 'Dossiers', 'Fichiers', 'Marque-pages', 'Historique'];

/** Sections dont un résultat est toujours le meilleur résultat quand il y en a un. */
const SECTIONS_D_APPLICATIONS: readonly string[] = ['Logiciels', 'Applications'];

/** Lignes montrées par section quand toutes sont mêlées ; au-delà, une ligne « Plus de… ». */
export const PAR_SECTION = 5;

/** Titres des sections qui ne sont pas des types de résultat. */
export const SECTION_FILTRES = 'Filtres';
export const SECTION_MEILLEUR = 'Meilleur résultat';

/**
 * Une ligne de la liste : un filtre à poser (champ vide, ou proposé par le mot tapé), un résultat,
 * ou « Plus de… » au bout d'une section, qui pose le filtre de cette section.
 */
export type LigneListe =
  { sorte: 'filtre'; filtre: NomFiltre } | { sorte: 'resultat'; resultat: Resultat } | { sorte: 'plus'; filtre: NomFiltre; section: string };

/** Une section de la liste : son titre et le rang de sa première ligne. */
export interface Section {
  titre: string;
  debut: number;
}

/** Résultats rangés par section, dans l'ordre fixe ; chaque ligne garde son rang dans sa section. */
export function grouper(resultats: readonly Resultat[]): Map<string, Resultat[]> {
  const parSection = new Map<string, Resultat[]>();
  for (const resultat of resultats) {
    const lignes = parSection.get(resultat.groupe);
    if (lignes === undefined) {
      parSection.set(resultat.groupe, [resultat]);
    } else {
      lignes.push(resultat);
    }
  }
  const rang = (titre: string): number => {
    const trouve = ORDRE_DES_SECTIONS.indexOf(titre);
    return trouve === -1 ? ORDRE_DES_SECTIONS.length : trouve;
  };
  return new Map([...parSection].sort(([a], [b]) => rang(a) - rang(b)));
}

/**
 * Lignes de la liste selon ce qu'elle montre (spécification §6) :
 * - `filtres` (champ vide sans filtre) : les filtres de genre à choisir ;
 * - `complet` (un filtre ou un dossier) : toutes les lignes, par section, ou en une seule section
 *   titrée `titreUnique` (les récents, dans l'ordre où ils ont été ouverts) ;
 * - `resume` : le meilleur résultat en tête, puis `PAR_SECTION` lignes par section suivies de
 *   « Plus de… » quand il y en a d'autres.
 * Les filtres `proposes` par le mot tapé viennent avant tout, dans leur section.
 */
export function construireLignes(
  mode: 'filtres' | 'complet' | 'resume',
  resultats: readonly Resultat[],
  proposes: readonly NomFiltre[] = [],
  titreUnique: string | null = null,
): { lignes: LigneListe[]; sections: Section[] } {
  if (mode === 'filtres') {
    return { lignes: FILTRES.filter((filtre) => filtre.sorte === 'genre').map((filtre) => ({ sorte: 'filtre', filtre: filtre.nom })), sections: [] };
  }
  const lignes: LigneListe[] = [];
  const sections: Section[] = [];
  const ajouterSection = (titre: string, membres: readonly LigneListe[]): void => {
    if (membres.length > 0) {
      sections.push({ titre, debut: lignes.length });
      lignes.push(...membres);
    }
  };
  ajouterSection(
    SECTION_FILTRES,
    proposes.map((filtre) => ({ sorte: 'filtre', filtre })),
  );
  const enLigne = (resultat: Resultat): LigneListe => ({ sorte: 'resultat', resultat });
  if (titreUnique !== null) {
    ajouterSection(titreUnique, resultats.map(enLigne));
    return { lignes, sections };
  }
  let restants = resultats;
  if (mode === 'resume' && resultats.length > 1) {
    // Un logiciel ou une application trouvé passe avant une page ou un fichier de meilleur score :
    // taper « claude » doit proposer l'application, pas la page de Firefox qui en parle.
    const rangDuMeilleur = Math.max(
      0,
      resultats.findIndex((r) => SECTIONS_D_APPLICATIONS.includes(r.groupe)),
    );
    ajouterSection(SECTION_MEILLEUR, [enLigne(resultats[rangDuMeilleur]!)]);
    restants = resultats.filter((_, rang) => rang !== rangDuMeilleur);
  }
  for (const [titre, membres] of grouper(restants)) {
    const montres = mode === 'resume' ? membres.slice(0, PAR_SECTION) : membres;
    const filtre = filtreDeSection(titre);
    const plus: LigneListe[] = mode === 'resume' && membres.length > PAR_SECTION && filtre !== null ? [{ sorte: 'plus', filtre, section: titre }] : [];
    ajouterSection(titre, [...montres.map(enLigne), ...plus]);
  }
  return { lignes, sections };
}

/** Première ligne de la section suivante (ou précédente), en boucle. */
export function sectionVoisine(active: number, sections: readonly Section[], sens: 1 | -1): number {
  if (sections.length === 0) {
    return active;
  }
  let courante = 0;
  sections.forEach((section, rang) => {
    if (section.debut <= active) {
      courante = rang;
    }
  });
  const voisine = (courante + sens + sections.length) % sections.length;
  return sections[voisine]!.debut;
}
