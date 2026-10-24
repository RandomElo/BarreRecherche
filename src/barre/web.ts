/**
 * Adresse tapée dans la barre (spécification §6) : « github.com/x », « https://… ». Même règle que
 * `adresse_tapee` du Rust (`lancement.rs`), qui la vérifie de nouveau avant d'ouvrir Firefox.
 */

/** Domaines de premier niveau qui font d'un texte sans `http` une adresse : une liste courte, pour qu'un nom de fichier (« rapport.pdf ») n'en soit pas une. */
const DOMAINES_RECONNUS = [
  'com',
  'fr',
  'org',
  'net',
  'io',
  'dev',
  'app',
  'eu',
  'be',
  'ch',
  'ca',
  'de',
  'uk',
  'co',
  'info',
  'me',
  'ai',
  'gg',
  'tv',
  'xyz',
  'edu',
  'gov',
];

/** L'adresse telle que tapée si elle en a la forme, sinon `null`. */
export function adresseTapee(texte: string): string | null {
  const adresse = texte.trim();
  // eslint-disable-next-line no-control-regex
  if (adresse === '' || adresse.length > 2000 || /[\s\u0000-\u001f\u007f]/u.test(adresse)) {
    return null;
  }
  const minuscule = adresse.toLowerCase();
  if (minuscule.startsWith('https://') || minuscule.startsWith('http://')) {
    return adresse.split('://')[1] === '' ? null : adresse;
  }
  const [hoteEtPort = ''] = minuscule.split('/');
  const [hote = '', port, ...trop] = hoteEtPort.split(':');
  if (trop.length > 0 || (port !== undefined && !/^\d+$/u.test(port))) {
    return null;
  }
  const etiquettes = hote.split('.');
  const valides = etiquettes.length >= 2 && etiquettes.every((e) => /^[a-z0-9-]+$/u.test(e));
  const reconnu = etiquettes[0] === 'www' || DOMAINES_RECONNUS.includes(etiquettes.at(-1) ?? '');
  return valides && reconnu ? adresse : null;
}
