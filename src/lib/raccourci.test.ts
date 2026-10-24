import { describe, expect, it } from 'vitest';
import { avertissementRaccourci } from './raccourci';

describe('avertissementRaccourci', () => {
  it('ne dit rien quand le raccourci marche', () => {
    expect(avertissementRaccourci('Ctrl+K', true)).toBeNull();
  });

  it('nomme le raccourci déjà pris', () => {
    expect(avertissementRaccourci('Ctrl+K', false)).toMatch(/^Ctrl\+K est déjà pris/);
  });
});
