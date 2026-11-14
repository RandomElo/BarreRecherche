/** Ce que fait `Tab` dans la barre, selon ce qui est sous le curseur. */
export type ActionTab = 'filtre-de-la-ligne' | 'filtre-propose' | 'entrer-dans-le-dossier' | 'section';

export interface SituationTab {
  /** Maj est tenue : `Tab` revient en arrière, sans poser de filtre ni entrer. */
  maj: boolean;
  /** La ligne active est une ligne de filtre. */
  ligneFiltre: boolean;
  /** Un filtre est proposé par le mot tapé (« vs » propose VS Code). */
  filtreProposé: boolean;
  /** Le résultat actif a des enfants : un dossier, une page de Notes avec des sous-pages. */
  resultatAvecEnfants: boolean;
}

/**
 * Ordre des choix : le filtre de la ligne, le filtre que propose le mot tapé, le dossier du
 * résultat actif (qui devient une étiquette, comme `→`), sinon la section suivante.
 */
export function actionTab(situation: SituationTab): ActionTab {
  if (!situation.maj) {
    if (situation.ligneFiltre) {
      return 'filtre-de-la-ligne';
    }
    if (situation.filtreProposé) {
      return 'filtre-propose';
    }
    if (situation.resultatAvecEnfants) {
      return 'entrer-dans-le-dossier';
    }
  }
  return 'section';
}

/**
 * Entrée avec un dossier en étiquette, le champ vide et un logiciel posé (« vsc » puis un dossier) :
 * ouvre ce dossier dans le logiciel, plutôt que son premier enfant.
 */
export function ouvreLeDossierChoisi(situation: { dossierEnEtiquette: boolean; champVide: boolean; logicielPose: boolean }): boolean {
  return situation.dossierEnEtiquette && situation.champVide && situation.logicielPose;
}
