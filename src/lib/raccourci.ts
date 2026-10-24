/** Avertissement quand le raccourci global n'a pas pu être pris, `null` s'il marche. */
export function avertissementRaccourci(raccourci: string, actif: boolean): string | null {
  if (actif) {
    return null;
  }
  return `${raccourci} est déjà pris par un autre programme, sans doute un raccourci de Cinnamon : le retirer, puis relancer la barre.`;
}
