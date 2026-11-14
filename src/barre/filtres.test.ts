import { describe, expect, it } from 'vitest';
import { filtreDuSysteme, filtreTape, filtresProposes, poser, pourLeSysteme, propositions, sansMotEnArobase } from './filtres';

const noms = (texte: string) => propositions(texte)?.map((filtre) => filtre.nom) ?? null;

describe('filtresProposes', () => {
  it('le seul mot du champ propose les filtres dont un nom commence par lui', () => {
    expect(filtresProposes('vs', [])).toEqual(['vsc']);
    expect(filtresProposes('dos', [])).toEqual(['dossiers']);
    expect(filtresProposes('fi', [])).toEqual(['fichiers', 'firefox']);
    expect(filtresProposes('Ter', [])).toEqual(['terminal']);
    // Autres noms et accents : « code » pour VS Code, « recents » pour Récents, « recherche » pour le Web.
    expect(filtresProposes('cod', [])).toEqual(['vsc']);
    expect(filtresProposes('réce', [])).toEqual(['recents']);
    expect(filtresProposes('rec', [])).toEqual(['recents', 'web']);
  });

  it('rien pour une lettre, plusieurs mots, un nom sans filtre, ou un filtre déjà posé', () => {
    expect(filtresProposes('v', [])).toEqual([]);
    expect(filtresProposes('vs code', [])).toEqual([]);
    expect(filtresProposes('rapport', [])).toEqual([]);
    expect(filtresProposes('vs', ['vsc'])).toEqual([]);
  });
});

describe('propositions', () => {
  it('« @ » seul propose tous les filtres, « @fi » ceux qui commencent ainsi', () => {
    expect(noms('@')).toHaveLength(14);
    expect(noms('rapport @fi')).toEqual(['fichiers', 'firefox']);
  });

  it('rien quand le mot n’est pas en @ ou ne mène à aucun filtre', () => {
    expect(noms('rapport')).toBeNull();
    expect(noms('eloi@exemple')).toBeNull();
    expect(noms('@zzz')).toBeNull();
    expect(noms('@vsc rapport')).toBeNull();
  });
});

describe('filtreTape', () => {
  it('un nom complet en tête suivi d’une espace, avec ou sans @, devient étiquette', () => {
    expect(filtreTape('vsc rapport')).toEqual({ filtre: 'vsc', reste: 'rapport' });
    expect(filtreTape('dossier ')).toEqual({ filtre: 'dossiers', reste: '' });
    expect(filtreTape('logiciels ')).toEqual({ filtre: 'logiciels', reste: '' });
    expect(filtreTape('@ter rapport')).toEqual({ filtre: 'terminal', reste: 'rapport' });
  });

  it('un nom complet en @ en fin de saisie devient étiquette', () => {
    expect(filtreTape('rapport @vsc')).toEqual({ filtre: 'vsc', reste: 'rapport ' });
    expect(filtreTape('@Notes')).toEqual({ filtre: 'notes', reste: '' });
  });

  it('rien tant que le nom n’est pas complet ou suivi d’une espace', () => {
    expect(filtreTape('vsc')).toBeNull();
    expect(filtreTape('rapport @vs')).toBeNull();
    expect(filtreTape('rapport vsc ')).toBeNull();
  });
});

describe('poser', () => {
  it('un filtre remplace celui de sa sorte ; un genre et un logiciel vont ensemble', () => {
    expect(poser(['vsc'], 'terminal')).toEqual(['terminal']);
    expect(poser(['vsc'], 'dossiers')).toEqual(['vsc', 'dossiers']);
    expect(poser(['notes'], 'dossiers')).toEqual(['dossiers']);
  });

  it('un filtre seul remplace tout, et part quand un autre est posé', () => {
    expect(poser(['vsc', 'dossiers'], 'bourse')).toEqual(['bourse']);
    expect(poser(['web'], 'logiciels')).toEqual(['logiciels']);
  });
});

describe('pourLeSysteme', () => {
  it('cibles du Rust et section du genre', () => {
    expect(pourLeSysteme(['vsc', 'fichiers'])).toEqual({ cibles: ['vsc', 'fichier'], section: 'Fichiers' });
    expect(pourLeSysteme(['notes'])).toEqual({ cibles: [], section: 'Notes' });
    expect(pourLeSysteme(['vsc'])).toEqual({ cibles: ['vsc'], section: null });
    expect(pourLeSysteme([])).toEqual({ cibles: [], section: null });
  });
});

describe('filtreDuSysteme', () => {
  it('nom du Rust (Ctrl+K depuis Notes, --cible=) vers le filtre', () => {
    expect(filtreDuSysteme('notes')).toBe('notes');
    expect(filtreDuSysteme('dossier')).toBe('dossiers');
    expect(filtreDuSysteme('vsc')).toBe('vsc');
    expect(filtreDuSysteme('inconnu')).toBeNull();
    expect(filtreDuSysteme(null)).toBeNull();
  });
});

describe('sansMotEnArobase', () => {
  it('retire le mot en @ de la fin', () => {
    expect(sansMotEnArobase('rapport @fi')).toBe('rapport ');
  });
});
