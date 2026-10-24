import { describe, expect, it } from 'vitest';
import type { Resultat } from '../lib/systeme';
import { CATEGORIES, PAR_SECTION, construireLignes, sectionVoisine } from './sections';

const r = (titre: string, groupe: string): Resultat => ({
  id: titre,
  titre,
  source: '',
  genre: 'fichier',
  groupe,
  positions: [],
  aUneIcone: false,
  aDesEnfants: false,
});

const decrire = (mode: 'categories' | 'complet' | 'resume', resultats: Resultat[]) =>
  construireLignes(mode, resultats).lignes.map((l) => (l.sorte === 'resultat' ? l.resultat.titre : `${l.sorte}:${l.categorie}`));

describe('construireLignes', () => {
  it('champ vide : les catégories, dans l’ordre', () => {
    expect(decrire('categories', [])).toEqual(CATEGORIES.map((c) => `categorie:${c}`));
  });

  it('range par section, la section du meilleur résultat d’abord, sans changer l’ordre dans une section', () => {
    const { lignes, sections } = construireLignes('complet', [r('a', 'Fichiers'), r('b', 'Applications'), r('c', 'Fichiers')]);
    expect(lignes.map((l) => (l.sorte === 'resultat' ? l.resultat.titre : l.sorte))).toEqual(['a', 'c', 'b']);
    expect(sections).toEqual([
      { titre: 'Fichiers', debut: 0 },
      { titre: 'Applications', debut: 2 },
    ]);
  });

  it('résumé : cinq lignes par section, puis « Plus de… » s’il y en a d’autres', () => {
    const fichiers = Array.from({ length: PAR_SECTION + 1 }, (_, i) => r(`f${i}`, 'Fichiers'));
    const lignes = decrire('resume', [...fichiers, r('app', 'Applications')]);
    expect(lignes).toEqual(['f0', 'f1', 'f2', 'f3', 'f4', 'plus:Fichiers', 'app']);
    expect(decrire('complet', fichiers)).toHaveLength(PAR_SECTION + 1);
  });
});

describe('sectionVoisine', () => {
  const sections = [
    { titre: 'Fichiers', debut: 0 },
    { titre: 'Applications', debut: 2 },
    { titre: 'Historique', debut: 3 },
  ];

  it('va au début de la section suivante ou précédente, en boucle', () => {
    expect(sectionVoisine(1, sections, 1)).toBe(2);
    expect(sectionVoisine(3, sections, 1)).toBe(0);
    expect(sectionVoisine(1, sections, -1)).toBe(3);
    expect(sectionVoisine(2, sections, -1)).toBe(0);
  });

  it('ne bouge pas sans section', () => {
    expect(sectionVoisine(4, [], 1)).toBe(4);
  });
});
