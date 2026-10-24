import { icons, type LucideIcon } from 'lucide-react';

/** Les icônes Lucide par leur nom sans tirets ni majuscules, comme dans Notes (`PageIcon.tsx`). */
const parNom = new Map(Object.entries(icons as Record<string, LucideIcon>).map(([nom, composant]) => [nom.toLowerCase(), composant]));

/**
 * Composant de l'icône Lucide écrite `book-open` dans le front-matter d'une page de Notes, ou
 * `null` si elle n'existe pas. Ce module embarque toutes les icônes : la barre ne le charge
 * qu'au premier besoin.
 */
export function composantLucide(nom: string): LucideIcon | null {
  return parNom.get(nom.replace(/-/g, '').toLowerCase()) ?? null;
}
