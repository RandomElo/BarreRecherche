import type { Resultat } from '../lib/systeme';

/** Catégories de la barre (spécification §6), proposées au champ vide ; noms partagés avec le Rust (`cibles::SECTIONS`). */
export const CATEGORIES = ['Récents', 'Logiciels', 'Applications', 'Notes', 'Dossiers', 'Fichiers', 'Marque-pages', 'Historique'] as const;
export type Categorie = (typeof CATEGORIES)[number];

/** Lignes montrées par section quand toutes sont mêlées ; au-delà, une ligne « Plus de… ». */
export const PAR_SECTION = 5;

/** Une ligne de la liste : une catégorie à choisir, un résultat, ou « Plus de… » au bout d'une section. */
export type LigneListe = { sorte: 'categorie'; categorie: Categorie } | { sorte: 'resultat'; resultat: Resultat } | { sorte: 'plus'; categorie: Categorie };

/** Une section de la liste : son titre et le rang de sa première ligne. */
export interface Section {
  titre: string;
  debut: number;
}

export function estUneCategorie(nom: string): nom is Categorie {
  return (CATEGORIES as readonly string[]).includes(nom);
}

/**
 * Résultats rangés par section (Applications, Dossiers, Fichiers, Marque-pages, Historique) :
 * les sections viennent dans l'ordre de leur meilleur résultat, chaque ligne garde son rang dans
 * sa section.
 */
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
  return parSection;
}

/**
 * Lignes de la liste selon ce qu'elle montre : les catégories (champ vide), toutes les lignes par
 * section (dans une catégorie ou un dossier), ou `PAR_SECTION` lignes par section suivies de
 * « Plus de… » quand il y en a d'autres.
 */
export function construireLignes(mode: 'categories' | 'complet' | 'resume', resultats: readonly Resultat[]): { lignes: LigneListe[]; sections: Section[] } {
  if (mode === 'categories') {
    return { lignes: CATEGORIES.map((categorie) => ({ sorte: 'categorie', categorie })), sections: [] };
  }
  const lignes: LigneListe[] = [];
  const sections: Section[] = [];
  for (const [titre, membres] of grouper(resultats)) {
    sections.push({ titre, debut: lignes.length });
    const montres = mode === 'resume' ? membres.slice(0, PAR_SECTION) : membres;
    lignes.push(...montres.map((resultat): LigneListe => ({ sorte: 'resultat', resultat })));
    if (mode === 'resume' && membres.length > PAR_SECTION && estUneCategorie(titre)) {
      lignes.push({ sorte: 'plus', categorie: titre });
    }
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
