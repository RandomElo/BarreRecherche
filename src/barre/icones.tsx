import { AppWindow, ChartLine, Code, File, Folder, Globe, type LucideIcon, NotebookText, Search, SquareTerminal } from 'lucide-react';
import { useEffect, useState } from 'react';
import { systeme, type Resultat } from '../lib/systeme';
import type { NomCible } from './cibles';

/** Icônes déjà demandées, pour la durée de vie de la page : `null` quand il n'y en a pas. */
const connues = new Map<string, string | null>();

/** Adresse `data:` de l'icône de cet identifiant (résultat, ou `cible:<nom>`), demandée une fois. */
export function useIcone(id: string, aDemander: boolean): string | null {
  const [adresse, definirAdresse] = useState(() => connues.get(id) ?? null);
  useEffect(() => {
    if (!aDemander || connues.has(id)) {
      definirAdresse(connues.get(id) ?? null);
      return;
    }
    let actif = true;
    void systeme.icone(id).then((trouvee) => {
      connues.set(id, trouvee);
      if (actif) {
        definirAdresse(trouvee);
      }
    });
    return () => {
      actif = false;
    };
  }, [id, aDemander]);
  return adresse;
}

const parGenre: Record<Resultat['genre'], LucideIcon> = { application: AppWindow, fichier: File, dossier: Folder, page: Globe, note: NotebookText };
const parCible: Record<NomCible, LucideIcon> = {
  vsc: Code,
  terminal: SquareTerminal,
  firefox: Globe,
  notes: NotebookText,
  fichier: File,
  dossier: Folder,
  bourse: ChartLine,
  web: Search,
  google: Search,
};

/** Icône Lucide d'une page de Notes, une fois le module des icônes chargé ; l'icône simple d'ici là. */
function IconeLucide({ nom, taille }: { nom: string; taille: string }) {
  const [Icone, definirIcone] = useState<LucideIcon | null>(null);
  useEffect(() => {
    let actif = true;
    void import('./lucide').then(({ composantLucide }) => {
      const trouvee = composantLucide(nom);
      if (actif && trouvee !== null) {
        definirIcone(() => trouvee);
      }
    });
    return () => {
      actif = false;
    };
  }, [nom]);
  const Montree = Icone ?? NotebookText;
  return <Montree className={`text-discret ${taille} shrink-0`} aria-hidden="true" />;
}

/**
 * Icône d'un résultat : celle du système (application, type de fichier, site, image d'une page de
 * Notes), l'emoji ou l'icône Lucide d'une page de Notes, sinon selon le genre.
 */
export function IconeResultat({ resultat, taille = 'size-5' }: { resultat: Resultat; taille?: string }) {
  const adresse = useIcone(resultat.id, resultat.aUneIcone);
  if (resultat.pictogramme?.startsWith('emoji:') === true) {
    return (
      <span className={`flex ${taille} shrink-0 items-center justify-center leading-none ${taille === 'size-5' ? 'text-base' : 'text-xs'}`} aria-hidden="true">
        {resultat.pictogramme.slice('emoji:'.length)}
      </span>
    );
  }
  if (resultat.pictogramme?.startsWith('lucide:') === true) {
    return <IconeLucide nom={resultat.pictogramme.slice('lucide:'.length)} taille={taille} />;
  }
  const ParDefaut = parGenre[resultat.genre];
  return adresse === null ? (
    <ParDefaut className={`text-discret ${taille} shrink-0`} aria-hidden="true" />
  ) : (
    <img src={adresse} alt="" className={`${taille} shrink-0 object-contain`} draggable={false} />
  );
}

/** Icône d'une cible : celle du logiciel installé (VS Code, Ghostty, Firefox), sinon une icône simple. */
export function IconeCible({ nom, taille = 'size-5' }: { nom: NomCible; taille?: string }) {
  const adresse = useIcone(`cible:${nom}`, true);
  const ParDefaut = parCible[nom];
  return adresse === null ? (
    <ParDefaut className={`text-discret ${taille} shrink-0`} aria-hidden="true" />
  ) : (
    <img src={adresse} alt="" className={`${taille} shrink-0 object-contain`} draggable={false} />
  );
}
