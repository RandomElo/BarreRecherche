import { describe, expect, it } from 'vitest';
import { actionDe, categorieTapee, cibleTapee, completion, nomDeCible, poser, propositions, sansMotEnArobase } from './cibles';

const noms = (texte: string) => propositions(texte)?.map((cible) => cible.nom) ?? null;

describe('propositions', () => {
  it('« @ » seul propose toutes les cibles', () => {
    expect(noms('@')).toEqual(['vsc', 'terminal', 'firefox', 'notes', 'fichier', 'dossier', 'bourse', 'web', 'google']);
    expect(noms('rapport @')).toEqual(['vsc', 'terminal', 'firefox', 'notes', 'fichier', 'dossier', 'bourse', 'web', 'google']);
  });

  it('filtre sur le début du nom, sans tenir compte de la casse', () => {
    expect(noms('rapport @fi')).toEqual(['firefox', 'fichier']);
    expect(noms('@T')).toEqual(['terminal']);
  });

  it('rien quand le mot n’est pas en @ ou ne mène à aucune cible', () => {
    expect(noms('rapport')).toBeNull();
    expect(noms('eloi@exemple')).toBeNull();
    expect(noms('@zzz')).toBeNull();
    expect(noms('@vsc rapport')).toBeNull();
  });
});

describe('cibleTapee', () => {
  it('une cible tapée en entier devient étiquette', () => {
    expect(cibleTapee('rapport @vsc')).toEqual({ cible: 'vsc', reste: 'rapport ' });
    expect(cibleTapee('@Terminal')).toEqual({ cible: 'terminal', reste: '' });
  });

  it('rien tant que le nom n’est pas complet', () => {
    expect(cibleTapee('rapport @vs')).toBeNull();
    expect(cibleTapee('rapport')).toBeNull();
  });

  it('un début de nom suivi d’une espace devient étiquette', () => {
    expect(cibleTapee('@ter ')).toEqual({ cible: 'terminal', reste: '' });
    expect(cibleTapee('@ter rapport')).toEqual({ cible: 'terminal', reste: 'rapport' });
    expect(cibleTapee('rapport @do ')).toEqual({ cible: 'dossier', reste: 'rapport ' });
    expect(cibleTapee('@fi x')).toEqual({ cible: 'firefox', reste: 'x' });
  });

  it('sans @ : un nom complet en tête du champ, suivi d’une espace', () => {
    expect(cibleTapee('vsc rapport')).toEqual({ cible: 'vsc', reste: 'rapport' });
    expect(cibleTapee('Terminal ')).toEqual({ cible: 'terminal', reste: '' });
    expect(cibleTapee('bourse alstom')).toEqual({ cible: 'bourse', reste: 'alstom' });
    // Un mot seul reste une recherche, comme un début de nom ou un nom plus loin dans la saisie.
    expect(cibleTapee('vsc')).toBeNull();
    expect(cibleTapee('ter rapport')).toBeNull();
    expect(cibleTapee('rapport vsc ')).toBeNull();
  });

  it('code ouvre VS Code, ghostty le terminal, avec ou sans @', () => {
    expect(cibleTapee('code rapport')).toEqual({ cible: 'vsc', reste: 'rapport' });
    expect(cibleTapee('Ghostty projet')).toEqual({ cible: 'terminal', reste: 'projet' });
    expect(cibleTapee('rapport @code')).toEqual({ cible: 'vsc', reste: 'rapport ' });
    expect(cibleTapee('notes cours')).toEqual({ cible: 'notes', reste: 'cours' });
  });

  it('rien pour un @ seul, un mot inconnu ou une adresse', () => {
    expect(cibleTapee('@ rapport')).toBeNull();
    expect(cibleTapee('@zzz rapport')).toBeNull();
    expect(cibleTapee('eloi@vsc rapport')).toBeNull();
  });
});

describe('categorieTapee', () => {
  it('logiciel ou logiciels, avec ou sans @, en tête du champ', () => {
    expect(categorieTapee('logiciel ')).toBe('');
    expect(categorieTapee('Logiciels analyse')).toBe('analyse');
    expect(categorieTapee('@logiciel ana')).toBe('ana');
    expect(categorieTapee('logiciel')).toBeNull();
    expect(categorieTapee('mes logiciels ')).toBeNull();
  });
});

describe('sansMotEnArobase', () => {
  it('retire le mot en @ de la fin seulement', () => {
    expect(sansMotEnArobase('rapport @fi')).toBe('rapport ');
    expect(sansMotEnArobase('@')).toBe('');
    expect(sansMotEnArobase('a@b')).toBe('a@b');
  });
});

describe('nomDeCible', () => {
  it('ne garde que les noms connus', () => {
    expect(nomDeCible('vsc')).toBe('vsc');
    expect(nomDeCible('notes')).toBe('notes');
    expect(nomDeCible('note')).toBeNull();
    expect(nomDeCible(null)).toBeNull();
  });
});

describe('poser', () => {
  it('un logiciel et un genre à la fois, le nouveau remplace celui de sa sorte', () => {
    expect(poser([], 'vsc')).toEqual(['vsc']);
    expect(poser(['vsc'], 'dossier')).toEqual(['vsc', 'dossier']);
    expect(poser(['vsc', 'dossier'], 'terminal')).toEqual(['dossier', 'terminal']);
    expect(poser(['vsc', 'dossier'], 'fichier')).toEqual(['vsc', 'fichier']);
  });

  it('@bourse se pose seule', () => {
    expect(poser(['vsc', 'dossier'], 'bourse')).toEqual(['bourse']);
    expect(poser(['bourse'], 'dossier')).toEqual(['dossier']);
    expect(cibleTapee('@bourse alstom')).toEqual({ cible: 'bourse', reste: 'alstom' });
    expect(cibleTapee('alstom @bourse')).toEqual({ cible: 'bourse', reste: 'alstom ' });
  });

  it('l’action est celle du logiciel', () => {
    expect(actionDe(['dossier', 'vsc'])).toBe('ouvrir dans VS Code');
    expect(actionDe(['dossier'])).toBe('ouvrir le dossier');
  });
});

describe('completion', () => {
  it('complète le premier mot vers une cible, un autre nom ou « logiciels »', () => {
    expect(completion('vs', [])).toEqual({ nom: 'vsc', suite: 'c', texte: 'vsc ', filtre: '@vsc' });
    expect(completion('Te', [])).toEqual({ nom: 'terminal', suite: 'rminal', texte: 'terminal ', filtre: '@terminal' });
    expect(completion('co', [])).toEqual({ nom: 'code', suite: 'de', texte: 'code ', filtre: '@vsc' });
    expect(completion('lo', [])).toEqual({ nom: 'logiciels', suite: 'giciels', texte: 'logiciels ', filtre: 'Logiciels' });
    expect(completion('@lo', [])?.texte).toBe('logiciels ');
    expect(completion('vsc', [])?.suite).toBe('');
  });

  it('deux lettres sans @, une avec', () => {
    expect(completion('v', [])).toBeNull();
    expect(completion('@g', [])?.nom).toBe('google');
    expect(completion('@gh', [])?.nom).toBe('ghostty');
  });

  it('rien après une espace, sur un mot inconnu, ou pour une cible déjà posée', () => {
    expect(completion('vsc rapport', [])).toBeNull();
    expect(completion('rapport no', [])).toBeNull();
    expect(completion('zz', [])).toBeNull();
    expect(completion('fi', ['firefox'])?.nom).toBe('fichier');
    expect(completion('co', ['vsc'])).toBeNull();
  });

  it('la saisie complétée devient bien une étiquette', () => {
    expect(cibleTapee(completion('co', [])!.texte)).toEqual({ cible: 'vsc', reste: '' });
    expect(categorieTapee(completion('lo', [])!.texte)).toBe('');
  });
});

describe('@web', () => {
  it('se pose seule, par son nom ou par « recherche »', () => {
    expect(poser(['vsc', 'dossier'], 'web')).toEqual(['web']);
    expect(cibleTapee('recherche cve php')).toEqual({ cible: 'web', reste: 'cve php' });
    expect(completion('rech', [])?.filtre).toBe('@web');
    expect(cibleTapee('google cve php')).toEqual({ cible: 'google', reste: 'cve php' });
    expect(poser(['web'], 'google')).toEqual(['google']);
    expect(cibleTapee('chrome cve php')).toEqual({ cible: 'google', reste: 'cve php' });
    expect(completion('chr', [])?.filtre).toBe('@google');
  });
});
