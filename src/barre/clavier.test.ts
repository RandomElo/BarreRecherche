import { describe, expect, it } from 'vitest';
import { actionTab, ouvreLeDossierChoisi } from './clavier';

const rien = { maj: false, ligneFiltre: false, filtreProposé: false, resultatAvecEnfants: false };

describe('actionTab', () => {
  it('pose le filtre de la ligne active', () => {
    expect(actionTab({ ...rien, ligneFiltre: true, filtreProposé: true })).toBe('filtre-de-la-ligne');
  });

  it('pose le filtre que propose le mot tapé (« vsc »)', () => {
    expect(actionTab({ ...rien, filtreProposé: true, resultatAvecEnfants: true })).toBe('filtre-propose');
  });

  it('entre dans le dossier actif (« Finger »)', () => {
    expect(actionTab({ ...rien, resultatAvecEnfants: true })).toBe('entrer-dans-le-dossier');
  });

  it('passe à la section suivante sur un résultat sans enfants', () => {
    expect(actionTab(rien)).toBe('section');
  });

  it('avec Maj, revient en arrière sans rien poser', () => {
    expect(actionTab({ maj: true, ligneFiltre: true, filtreProposé: true, resultatAvecEnfants: true })).toBe('section');
  });
});

describe('ouvreLeDossierChoisi', () => {
  it('ouvre le dossier quand le champ est vide et un logiciel posé', () => {
    expect(ouvreLeDossierChoisi({ dossierEnEtiquette: true, champVide: true, logicielPose: true })).toBe(true);
  });

  it('laisse Entrée ouvrir la ligne active sinon', () => {
    expect(ouvreLeDossierChoisi({ dossierEnEtiquette: true, champVide: false, logicielPose: true })).toBe(false);
    expect(ouvreLeDossierChoisi({ dossierEnEtiquette: true, champVide: true, logicielPose: false })).toBe(false);
    expect(ouvreLeDossierChoisi({ dossierEnEtiquette: false, champVide: true, logicielPose: true })).toBe(false);
  });
});
