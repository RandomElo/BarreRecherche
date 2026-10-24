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
