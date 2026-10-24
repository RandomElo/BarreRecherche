/** Mise en forme des chiffres de `@bourse` (spécification §4, J10) et tracé de la courbe. */

/** Montant dans sa devise (« 21,50 € », « 10 500 € ») ; sans devise connue, le nombre seul. `signe` : « +12,30 € ». */
export function montant(valeur: number, devise: string, signe = false): string {
  const chiffres = Math.abs(valeur) >= 1000 ? 0 : 2;
  const signDisplay = signe ? 'exceptZero' : 'auto';
  try {
    return new Intl.NumberFormat('fr-FR', {
      style: 'currency',
      currency: devise,
      signDisplay,
      minimumFractionDigits: chiffres,
      maximumFractionDigits: chiffres,
    }).format(valeur);
  } catch {
    return new Intl.NumberFormat('fr-FR', { signDisplay, maximumFractionDigits: 4 }).format(valeur);
  }
}

/** Pourcentage signé : « +1,20 % », « −0,80 % ». */
export function pourcentage(valeur: number): string {
  return new Intl.NumberFormat('fr-FR', { style: 'percent', signDisplay: 'exceptZero', minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(
    valeur / 100,
  );
}

/** Classe de couleur d'une variation : hausse en vert, baisse en rouge. */
export function couleur(valeur: number | null): string {
  return valeur === null || valeur === 0 ? 'text-discret' : valeur > 0 ? 'text-hausse' : 'text-baisse';
}

/**
 * Chemins SVG des séries, à la même échelle, dans un cadre `largeur` × `hauteur` : la première
 * série donne l'axe du temps, les autres sont tracées sur autant de points.
 */
export function tracer(series: readonly (readonly number[])[], largeur: number, hauteur: number): string[] {
  const toutes = series.flat();
  if (toutes.length === 0) {
    return series.map(() => '');
  }
  const bas = Math.min(...toutes);
  const haut = Math.max(...toutes);
  const etendue = haut - bas || 1;
  return series.map((serie) => {
    const pas = serie.length > 1 ? largeur / (serie.length - 1) : 0;
    return serie
      .map((valeur, rang) => {
        const x = Math.round(rang * pas * 10) / 10;
        const y = Math.round((hauteur - ((valeur - bas) / etendue) * hauteur) * 10) / 10;
        return `${rang === 0 ? 'M' : 'L'}${x},${y}`;
      })
      .join(' ');
  });
}

/** Heure d'une barre (`2026-10-09T07:00:00.000Z` → « 09:00 » à Paris), ou « maintenant ». */
export function heure(horodatage: string, fuseau?: string): string {
  const date = new Date(horodatage);
  return Number.isNaN(date.getTime()) ? horodatage : new Intl.DateTimeFormat('fr-FR', { hour: '2-digit', minute: '2-digit', timeZone: fuseau }).format(date);
}

/** Vrai si la courbe est celle du jour même ; sinon c'est la séance de la veille (marché fermé, week-end). */
export function estAujourdhui(jour: string | null, maintenant = new Date()): boolean {
  return jour === null || jour === maintenant.toISOString().slice(0, 10);
}

/** Jour de la courbe : « aujourd'hui », sinon « séance de la veille, vendredi 9 octobre », pour qu'on ne la prenne pas pour celle du jour. */
export function libelleDuJour(jour: string | null, maintenant = new Date()): string {
  if (estAujourdhui(jour, maintenant)) {
    return 'aujourd’hui';
  }
  return `séance de la veille, ${new Date(`${jour}T12:00:00Z`).toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', timeZone: 'UTC' })}`;
}
