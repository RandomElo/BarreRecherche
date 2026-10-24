import { invoke } from '@tauri-apps/api/core';

/** État lu par la barre et les réglages (src-tauri/src/reglages.rs). */
export interface EtatReglages {
  raccourci: string;
  raccourciActif: boolean;
  /** `null` hors AppImage : rien à lancer avec la session. */
  demarrageAuto: boolean | null;
}

/** Une ligne de la liste (src-tauri/src/recherche.rs). */
export interface Resultat {
  id: string;
  titre: string;
  /** Source dite à droite de la ligne : « Application », ou l'emplacement d'un fichier (`~/Documents`). */
  source: string;
  /** Choisit l'icône par défaut. */
  genre: 'application' | 'fichier' | 'dossier' | 'page' | 'note';
  /** Section de la liste : Applications, Notes, Dossiers, Fichiers, Marque-pages, Historique. */
  groupe: string;
  /** Position, en caractères du titre, des lettres trouvées. */
  positions: number[];
  aUneIcone: boolean;
  /** Icône d'une page de Notes à dessiner ici : `emoji:📚` ou `lucide:book-open`. */
  pictogramme?: string;
  /** `→` entre dedans : un dossier, ou une page de Notes qui a des sous-pages. */
  aDesEnfants: boolean;
}

/** Ouverture par `Ctrl+K` (src-tauri/src/contexte.rs) : cible du logiciel au premier plan. */
export interface Ouverture {
  /** Nom de la cible, sans le `@`. */
  cible: string | null;
  /** Ouverte depuis VS Code : `Ctrl+O` rend `Ctrl+K Ctrl+O` à VS Code. */
  relais: boolean;
}

/** Aperçu de la ligne active (src-tauri/src/apercu.rs). Le texte est à afficher tel quel, jamais en HTML. */
export interface Entete {
  titre: string;
  details: string;
}

export type Apercu =
  | { genre: 'texte'; entete: Entete; texte: string; tronque: boolean }
  | { genre: 'image'; entete: Entete; adresse: string }
  | { genre: 'fichier'; entete: Entete }
  | { genre: 'html'; entete: Entete; adresse: string }
  | { genre: 'pdf'; entete: Entete }
  | { genre: 'dossier'; entete: Entete; elements: { nom: string; dossier: boolean; icone: string; indexe: boolean }[]; total: number }
  | { genre: 'note'; entete: Entete; enfants: Resultat[]; total: number; texte: string; tronque: boolean }
  | { genre: 'application'; entete: Entete; description: string | null }
  | { genre: 'page'; entete: Entete; adresse: string };

/** Une ligne de `@bourse` (src-tauri/src/bourse.rs) : action suivie, position ou portefeuille. */
export interface LigneBourse {
  /** `portefeuille`, `action:<symbole>` ou `position:<rang>`. */
  id: string;
  titre: string;
  detail: string;
  /** Cours d'une action, valeur en euros d'une position ou du portefeuille. */
  valeur: number | null;
  devise: string;
  /** Variation du jour, en %. */
  variationPct: number | null;
  /** Argent gagné ou perdu aujourd'hui, dans `devise` (par action pour une action). */
  gain: number | null;
  positions: number[];
}

/** Aperçu d'une ligne de `@bourse` : la valeur, le gain de la journée et sa courbe. */
export interface ApercuBourse {
  titre: string;
  details: string;
  valeur: number | null;
  devise: string;
  variationPct: number | null;
  gain: number | null;
  /** Jour de la courbe, en UTC (`2026-10-09`) : la dernière séance, aujourd'hui ou avant. */
  jour: string | null;
  /** Barres de 15 min (`heure` en UTC) ; marché ouvert, la dernière est le cours du moment (`maintenant`). */
  points: { heure: string; valeur: number }[];
}

export const systeme = {
  /** Hauteur du contenu ; la largeur est fixe. */
  ajusterTaille: (hauteur: number) => invoke<void>('ajuster_taille', { hauteur }),
  cacherBarre: () => invoke<void>('cacher_barre'),
  ouvrirReglages: () => invoke<void>('ouvrir_reglages'),
  etatReglages: () => invoke<EtatReglages>('etat_reglages'),
  definirDemarrageAuto: (actif: boolean) => invoke<EtatReglages>('definir_demarrage_auto', { actif }),
  quitter: () => invoke<void>('quitter'),
  /** `cibles` : noms sans le `@` (`vsc`, `dossier`…), un logiciel et un genre au plus. */
  /** `dossier` : identifiant du dossier où l'on est entré par → ; `section` : la catégorie choisie. */
  chercher: (saisie: string, cibles: string[], dossier: string | null, section: string | null) =>
    invoke<Resultat[]>('chercher', { saisie, cibles, dossier, section }),
  ouvrir: (id: string, cibles: string[]) => invoke<void>('ouvrir', { id, cibles }),
  /** `moteur` : `google`, sinon le moteur par défaut de Firefox. */
  chercherSurLeWeb: (texte: string, moteur: 'google' | null) => invoke<void>('chercher_sur_le_web', { texte, moteur }),
  /** Adresse tapée (« github.com »), ouverte dans un onglet de Firefox. */
  ouvrirSite: (adresse: string) => invoke<void>('ouvrir_site', { adresse }),
  /** Icône d'un résultat, ou d'une cible (`cible:vsc`). */
  icone: (id: string) => invoke<string | null>('icone', { id }),
  /** Lue une fois à chaque ouverture par `Ctrl+K`, `null` sinon. */
  contexteOuverture: () => invoke<Ouverture | null>('contexte_ouverture'),
  relayerCtrlKCtrlO: () => invoke<void>('relayer_ctrl_k_ctrl_o'),
  apercu: (id: string) => invoke<Apercu | null>('apercu', { id }),
  /** Octets du PDF, pour pdf.js. */
  apercuPdf: (id: string) => invoke<ArrayBuffer>('apercu_pdf', { id }),
  /** Portefeuille au champ vide, sinon les actions suivies qui correspondent. */
  bourseChercher: (saisie: string) => invoke<LigneBourse[]>('bourse_chercher', { saisie }),
  bourseApercu: (id: string) => invoke<ApercuBourse>('bourse_apercu', { id }),
};
