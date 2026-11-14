import { describe, expect, it } from 'vitest';
import type { Resultat } from '../lib/systeme';
import { PAR_SECTION, construireLignes, sectionVoisine } from './sections';

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

const decrire = (...args: Parameters<typeof construireLignes>) =>
  construireLignes(...args).lignes.map((l) => (l.sorte === 'resultat' ? l.resultat.titre : l.sorte === 'plus' ? `plus:${l.section}` : `filtre:${l.filtre}`));

describe('construireLignes', () => {
  it('champ vide : les filtres de genre, dans l’ordre', () => {
    expect(decrire('filtres', [])).toEqual([
      'filtre:recents',
      'filtre:logiciels',
      'filtre:applications',
      'filtre:notes',
      'filtre:dossiers',
      'filtre:fichiers',
      'filtre:marque-pages',
      'filtre:historique',
    ]);
  });

  it('sections dans un ordre fixe, quel que soit le meilleur résultat', () => {
    const { lignes, sections } = construireLignes('complet', [r('a', 'Fichiers'), r('b', 'Applications'), r('c', 'Fichiers')]);
    expect(lignes.map((l) => (l.sorte === 'resultat' ? l.resultat.titre : l.sorte))).toEqual(['b', 'a', 'c']);
    expect(sections).toEqual([
      { titre: 'Applications', debut: 0 },
      { titre: 'Fichiers', debut: 1 },
    ]);
  });

  it('résumé : le meilleur résultat en tête, puis cinq lignes par section et « Plus de… »', () => {
    const fichiers = Array.from({ length: PAR_SECTION + 2 }, (_, i) => r(`f${i}`, 'Fichiers'));
    const { lignes, sections } = construireLignes('resume', [...fichiers, r('app', 'Applications')]);
    expect(sections.map((s) => s.titre)).toEqual(['Meilleur résultat', 'Fichiers']);
    expect(decrire('resume', [...fichiers, r('app', 'Applications')])).toEqual(['app', 'f0', 'f1', 'f2', 'f3', 'f4', 'plus:Fichiers']);
    expect(lignes).toHaveLength(7);
    expect(decrire('complet', fichiers)).toHaveLength(PAR_SECTION + 2);
  });

  it('résumé : une application passe avant une page de meilleur score', () => {
    expect(decrire('resume', [r('Claude (page)', 'Historique'), r('Claude', 'Applications')])).toEqual(['Claude', 'Claude (page)']);
    expect(decrire('resume', [r('p', 'Historique'), r('f', 'Fichiers')])).toEqual(['p', 'f']);
  });

  it('les filtres proposés viennent en tête, dans leur section', () => {
    const { sections } = construireLignes('resume', [r('vk.json', 'Fichiers'), r('Visual Studio Code', 'Applications')], ['vsc']);
    expect(decrire('resume', [r('vk.json', 'Fichiers')], ['vsc'])).toEqual(['filtre:vsc', 'vk.json']);
    expect(sections[0]).toEqual({ titre: 'Filtres', debut: 0 });
  });

  it('les récents restent dans leur ordre, en une seule section', () => {
    expect(decrire('complet', [r('a', 'Fichiers'), r('b', 'Applications')], [], 'Récents')).toEqual(['a', 'b']);
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
