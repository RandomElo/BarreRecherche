import { describe, expect, it } from 'vitest';
import { adresseTapee } from './web';

describe('adresseTapee', () => {
  it('reconnaît une adresse, avec ou sans http', () => {
    expect(adresseTapee('github.com')).toBe('github.com');
    expect(adresseTapee(' www.exemple.test/a?b=c ')).toBe('www.exemple.test/a?b=c');
    expect(adresseTapee('localhost.dev:3000/x')).toBe('localhost.dev:3000/x');
    expect(adresseTapee('http://192.168.1.2:8080')).toBe('http://192.168.1.2:8080');
  });

  it('pas un nom de fichier, une recherche ou autre chose', () => {
    expect(adresseTapee('rapport.pdf')).toBeNull();
    expect(adresseTapee('cve php')).toBeNull();
    expect(adresseTapee('a..com')).toBeNull();
    expect(adresseTapee('javascript:alert(1)')).toBeNull();
    expect(adresseTapee('https://')).toBeNull();
    expect(adresseTapee('exemple.com:abc')).toBeNull();
  });
});
