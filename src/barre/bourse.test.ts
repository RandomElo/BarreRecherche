import { describe, expect, it } from 'vitest';
import { couleur, heure, estAujourdhui, libelleDuJour, montant, pourcentage, tracer } from './bourse';

// Intl met des espaces insécables : on les remplace pour comparer.
const simple = (texte: string) => texte.replace(/[\u00a0\u202f]/gu, ' ');

describe('mise en forme', () => {
  it('montants dans leur devise, sans centimes au-delà de mille', () => {
    expect(simple(montant(21.5, 'EUR'))).toBe('21,50 €');
    expect(simple(montant(10500, 'EUR'))).toBe('10 500 €');
    expect(simple(montant(180, 'USD'))).toBe('180,00 $US');
    expect(montant(3, '')).toBe('3');
  });

  it('pourcentages signés', () => {
    expect(simple(pourcentage(1.2))).toBe('+1,20 %');
    expect(simple(pourcentage(-0.8))).toMatch(/^[-−]0,80 %$/u);
    expect(simple(pourcentage(0))).toBe('0,00 %');
  });

  it('gain signé, couleur selon le sens', () => {
    expect(simple(montant(12.3, 'EUR', true))).toBe('+12,30 €');
    expect(simple(montant(-1250, 'EUR', true))).toMatch(/^[-−]1 250 €$/u);
    expect(montant(-3, '', true)).toMatch(/^[-−]3$/u);
    expect(couleur(2)).toBe('text-hausse');
    expect(couleur(-2)).toBe('text-baisse');
    expect(couleur(null)).toBe('text-discret');
  });
});

describe('tracer', () => {
  it('séries à la même échelle, la plus haute valeur en haut', () => {
    expect(tracer([[0, 10], [5]], 100, 50)).toEqual(['M0,50 L100,0', 'M0,25']);
  });

  it('série plate ou vide', () => {
    expect(tracer([[3, 3]], 10, 10)).toEqual(['M0,10 L10,10']);
    expect(tracer([[]], 10, 10)).toEqual(['']);
  });
});

describe('journée', () => {
  it('heure de Paris d’une barre en UTC', () => {
    expect(heure('2026-10-09T07:00:00.000Z', 'Europe/Paris')).toBe('09:00');
    expect(heure('maintenant')).toBe('maintenant');
  });

  it('aujourd’hui, sinon la séance de la veille avec son jour', () => {
    const dimanche = new Date('2026-10-11T10:00:00Z');
    expect(libelleDuJour('2026-10-11', dimanche)).toBe('aujourd’hui');
    expect(libelleDuJour('2026-10-09', dimanche)).toBe('séance de la veille, vendredi 9 octobre');
    expect(estAujourdhui('2026-10-09', dimanche)).toBe(false);
  });
});
