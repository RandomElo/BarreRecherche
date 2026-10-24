import { ChevronRight, File, Folder } from 'lucide-react';
import type { Apercu as DonneesApercu, Resultat } from '../lib/systeme';
import { ApercuPdf } from './ApercuPdf';
import { IconeResultat, useIcone } from './icones';

/** Largeur utile du panneau, en pixels : la barre large (1100) moins la liste (640) et les marges. */
export const LARGEUR_CONTENU = 428;

/** Panneau d'aperçu de la ligne active (spécification §6, J11). Tout le contenu est affiché en texte. */
export function Apercu({ apercu, resultat }: { apercu: DonneesApercu; resultat: Resultat }) {
  return (
    <div className="border-bordure flex h-full w-[460px] shrink-0 flex-col gap-3 overflow-hidden border-l p-4">
      <div className="flex min-w-0 items-center gap-3">
        <IconeResultat resultat={resultat} />
        <div className="min-w-0">
          <p className="truncate font-medium" title={apercu.entete.titre}>
            {apercu.entete.titre}
          </p>
          <p className="text-discret truncate text-xs" title={apercu.entete.details}>
            {apercu.entete.details}
          </p>
        </div>
      </div>
      <div className="border-bordure min-h-0 flex-1 overflow-hidden border-t pt-3">
        <Contenu apercu={apercu} id={resultat.id} />
      </div>
    </div>
  );
}

function Contenu({ apercu, id }: { apercu: DonneesApercu; id: string }) {
  switch (apercu.genre) {
    case 'texte':
      return (
        <pre className="text-texte/90 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap">
          {apercu.texte}
          {apercu.tronque && '\n…'}
        </pre>
      );
    case 'image':
      return <img src={apercu.adresse} alt="" className="max-h-full max-w-full rounded-sm object-contain" draggable={false} />;
    case 'fichier':
      return <p className="text-discret text-sm">Pas d’aperçu pour ce type de fichier.</p>;
    case 'html':
      return <ApercuHtml adresse={apercu.adresse} titre={apercu.entete.titre} />;
    case 'pdf':
      return <ApercuPdf id={id} largeur={LARGEUR_CONTENU} />;
    case 'dossier':
      return apercu.total === 0 ? (
        <p className="text-discret text-sm">Dossier vide</p>
      ) : (
        <ul className="flex flex-col gap-1 text-sm">
          {apercu.elements.map((element) => (
            <ElementDossier key={element.nom} element={element} />
          ))}
          {apercu.total > apercu.elements.length && <li className="text-discret text-xs">et {apercu.total - apercu.elements.length} autres</li>}
        </ul>
      );
    case 'note':
      return (
        <div className="flex h-full flex-col gap-3">
          {apercu.total > 0 && (
            <ul className="flex shrink-0 flex-col gap-1 text-sm">
              {apercu.enfants.map((page) => (
                <li key={page.id} className="flex min-w-0 items-center gap-2">
                  <IconeResultat resultat={page} taille="size-4" />
                  <span className="text-texte min-w-0 flex-1 truncate">{page.titre}</span>
                  {page.aDesEnfants && <ChevronRight className="text-discret size-3.5 shrink-0" aria-label="a des sous-pages" />}
                </li>
              ))}
              {apercu.total > apercu.enfants.length && <li className="text-discret text-xs">et {apercu.total - apercu.enfants.length} autres</li>}
            </ul>
          )}
          <pre
            className={`text-texte/90 min-h-0 font-mono text-xs leading-relaxed break-words whitespace-pre-wrap ${apercu.total > 0 ? 'border-bordure border-t pt-3' : ''}`}
          >
            {apercu.texte.trim() === '' ? (apercu.total > 0 ? '' : 'Page vide.') : apercu.texte}
            {apercu.tronque && '\n…'}
          </pre>
        </div>
      );
    case 'application':
      return <p className="text-sm">{apercu.description ?? 'Pas de description.'}</p>;
    case 'page':
      return <p className="text-discret text-sm break-all">{apercu.adresse}</p>;
  }
}

/**
 * Un élément d'un dossier : icône du thème selon son type (la même pour tous, indexés ou non),
 * dossiers en clair et un peu gras, fichiers en gris, « non indexé » à droite quand la barre ne le
 * trouve pas en recherche (dossier caché ou généré).
 */
function ElementDossier({ element }: { element: { nom: string; dossier: boolean; icone: string; indexe: boolean } }) {
  const icone = useIcone(`theme:${element.icone}`, element.icone !== '');
  const ParDefaut = element.dossier ? Folder : File;
  return (
    <li className="flex min-w-0 items-center gap-2">
      {icone === null ? (
        <ParDefaut className={`size-4 shrink-0 ${element.dossier ? 'text-accent' : 'text-discret'}`} aria-hidden="true" />
      ) : (
        <img src={icone} alt="" className="size-4 shrink-0 object-contain" draggable={false} />
      )}
      <span className={`min-w-0 flex-1 truncate ${element.dossier ? 'text-texte font-medium' : 'text-discret'}`}>{element.nom}</span>
      {!element.indexe && <span className="text-discret shrink-0 text-xs italic">non indexé</span>}
    </li>
  );
}

/** Échelle de la page HTML : rendue sur 820 px de large, réduite pour tenir dans le panneau. */
const ECHELLE_HTML = LARGEUR_CONTENU / 820;

/**
 * Page HTML rendue en vignette dans un cadre isolé : `sandbox` sans permission (ni script, ni
 * formulaire, origine à part), servie par `apercu://` avec sa propre CSP (ni script, ni réseau).
 */
function ApercuHtml({ adresse, titre }: { adresse: string; titre: string }) {
  if (!adresse.startsWith('apercu://')) {
    return null;
  }
  return (
    <div className="h-full overflow-hidden rounded-sm bg-white">
      <iframe
        src={adresse}
        title={`Aperçu de ${titre}`}
        sandbox=""
        referrerPolicy="no-referrer"
        tabIndex={-1}
        className="pointer-events-none origin-top-left border-0"
        style={{ width: 820, height: `${100 / ECHELLE_HTML}%`, transform: `scale(${ECHELLE_HTML})` }}
      />
    </div>
  );
}
