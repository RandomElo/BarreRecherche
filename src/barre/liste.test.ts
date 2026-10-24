import { describe, expect, it } from 'vitest';
import { decouper, ligneVoisine } from './liste';

describe('decouper', () => {
  it('regroupe les lettres trouvées qui se suivent', () => {
    expect(decouper('Firefox', [0, 1, 4])).toEqual([
      { texte: 'Fi', trouve: true },
      { texte: 're', trouve: false },
      { texte: 'f', trouve: true },
      { texte: 'ox', trouve: false },
    ]);
  });

  it('compte en caractères, même hors du plan de base', () => {
    expect(decouper('🎵 Musique', [2])).toEqual([
      { texte: '🎵 ', trouve: false },
      { texte: 'M', trouve: true },
      { texte: 'usique', trouve: false },
    ]);
  });

  it('sans position, un seul morceau', () => {
    expect(decouper('Terminal', [])).toEqual([{ texte: 'Terminal', trouve: false }]);
    expect(decouper('', [])).toEqual([]);
  });
});

describe('ligneVoisine', () => {
  it('avance et recule en boucle', () => {
    expect(ligneVoisine(0, 3, 1)).toBe(1);
    expect(ligneVoisine(2, 3, 1)).toBe(0);
    expect(ligneVoisine(0, 3, -1)).toBe(2);
  });

  it('liste vide', () => {
    expect(ligneVoisine(0, 0, 1)).toBe(0);
  });
});
