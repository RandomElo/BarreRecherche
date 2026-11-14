/** Morceau de titre, à mettre à la couleur d'accent s'il fait partie des lettres trouvées. */
export interface Morceau {
  texte: string;
  trouve: boolean;
}

/**
 * Découpe le titre en morceaux trouvés ou non. Les positions viennent du Rust et comptent des
 * caractères Unicode, pas des unités UTF-16 : d'où `Array.from`.
 */
export function decouper(titre: string, positions: readonly number[]): Morceau[] {
  const trouvees = new Set(positions);
  const morceaux: Morceau[] = [];
  Array.from(titre).forEach((caractere, position) => {
    const trouve = trouvees.has(position);
    const dernier = morceaux.at(-1);
    if (dernier !== undefined && dernier.trouve === trouve) {
      dernier.texte += caractere;
    } else {
      morceaux.push({ texte: caractere, trouve });
    }
  });
  return morceaux;
}

/** Ligne suivante (`sens` = 1) ou précédente (-1), en boucle (spécification §6). */
export function ligneVoisine(active: number, total: number, sens: 1 | -1): number {
  if (total === 0) {
    return 0;
  }
  return (active + sens + total) % total;
}

/** Caractères d'un emplacement qui tiennent à droite d'une ligne. */
export const LONGUEUR_EMPLACEMENT = 44;

/**
 * Emplacement coupé au milieu (spécification §6) : le début (`~`) et les derniers dossiers, ceux qui
 * distinguent deux fichiers de même nom (`~/…/EscapeGame/logs`). Un texte sans `/` reste tel quel.
 */
export function couperAuMilieu(emplacement: string, longueur = LONGUEUR_EMPLACEMENT): string {
  const parties = emplacement.split('/');
  if (Array.from(emplacement).length <= longueur || parties.length < 3) {
    return emplacement;
  }
  const debut = parties[0]!;
  const fin: string[] = [];
  let total = Array.from(debut).length + 2;
  for (let rang = parties.length - 1; rang > 0; rang--) {
    const partie = parties[rang]!;
    const taille = Array.from(partie).length + 1;
    if (fin.length > 0 && total + taille > longueur) {
      break;
    }
    fin.unshift(partie);
    total += taille;
  }
  return fin.length === parties.length - 1 ? emplacement : `${debut}/…/${fin.join('/')}`;
}
