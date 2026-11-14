import { describe, expect, it } from 'vitest';
import { couperAuMilieu, decouper, ligneVoisine } from './liste';

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

describe('couperAuMilieu', () => {
  it('garde le début et les derniers dossiers', () => {
    expect(couperAuMilieu('~/Documents/_École/2 BTS CIEL IR/EscapeGame/serveur/api/logs', 32)).toBe('~/…/EscapeGame/serveur/api/logs');
  });

  it('laisse un emplacement court, ou qui n’est pas un chemin', () => {
    expect(couperAuMilieu('~/Documents/Rapports', 30)).toBe('~/Documents/Rapports');
    expect(couperAuMilieu('Notes · Tâches › Claude › un titre très long qui dépasse', 20)).toBe('Notes · Tâches › Claude › un titre très long qui dépasse');
  });

  it('garde au moins le dernier dossier, même trop long', () => {
    expect(couperAuMilieu('~/a/b/un-dossier-au-nom-vraiment-trop-long', 10)).toBe('~/…/un-dossier-au-nom-vraiment-trop-long');
  });
});
