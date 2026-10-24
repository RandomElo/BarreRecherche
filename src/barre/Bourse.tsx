import { BriefcaseBusiness, ChartLine, Wallet } from 'lucide-react';
import { type MouseEvent } from 'react';
import type { ApercuBourse as DonneesApercu, LigneBourse as DonneesLigne } from '../lib/systeme';
import { LARGEUR_CONTENU } from './Apercu';
import { couleur, estAujourdhui, heure, libelleDuJour, montant, pourcentage, tracer } from './bourse';
import { decouper } from './liste';

function IconeLigne({ id, className }: { id: string; className: string }) {
  const Icone = id === 'portefeuille' ? Wallet : id.startsWith('position:') ? BriefcaseBusiness : ChartLine;
  return <Icone className={className} aria-hidden="true" />;
}

/** Une ligne de `@bourse` : nom et détail à gauche, variation du jour et argent gagné ou perdu à droite. */
export function LigneBourse({ ligne, active, onSurvoler }: { ligne: DonneesLigne; active: boolean; onSurvoler: (evenement: MouseEvent) => void }) {
  return (
    <li
      role="option"
      aria-selected={active}
      onMouseMove={onSurvoler}
      className={`flex h-12 cursor-default items-center gap-3 px-4 ${active ? 'bg-accent/25' : ''}`}
    >
      <IconeLigne id={ligne.id} className="text-accent size-5 shrink-0" />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate">
          {decouper(ligne.titre, ligne.positions).map((morceau, rang) => (
            <span key={rang} className={morceau.trouve ? 'text-accent' : undefined}>
              {morceau.texte}
            </span>
          ))}
        </span>
        <span className="text-discret truncate text-xs">{ligne.detail}</span>
      </span>
      <span className={`flex shrink-0 flex-col items-end tabular-nums ${couleur(ligne.variationPct ?? ligne.gain)}`}>
        <span className="font-medium">{ligne.variationPct === null ? '' : pourcentage(ligne.variationPct)}</span>
        <span className="text-xs">{ligne.gain === null ? '' : montant(ligne.gain, ligne.devise, true)}</span>
      </span>
    </li>
  );
}

const HAUTEUR_COURBE = 170;

/**
 * Aperçu d'une ligne de `@bourse`, à la façon de l'ancien Google Finance : la valeur en grand,
 * ce qui a été gagné ou perdu dans la journée, sa courbe (barres de 15 min) et, en pointillé, la
 * clôture précédente. Hors séance, la courbe est celle de la veille, et c'est écrit.
 */
export function ApercuBourse({ apercu, id }: { apercu: DonneesApercu; id: string }) {
  const valeurs = apercu.points.map((p) => p.valeur);
  const veille = apercu.valeur !== null && apercu.gain !== null ? apercu.valeur - apercu.gain : null;
  const [chemin = '', ligneVeille = ''] = tracer([valeurs, veille === null ? [] : [veille, veille]], LARGEUR_CONTENU, HAUTEUR_COURBE);
  const sens = apercu.gain ?? apercu.variationPct;
  const premier = apercu.points[0];
  const dernier = apercu.points.at(-1);
  const jour = libelleDuJour(apercu.jour);
  return (
    <div className="border-bordure flex h-full w-[460px] shrink-0 flex-col gap-4 overflow-hidden border-l p-4">
      <div className="flex min-w-0 items-center gap-3">
        <IconeLigne id={id} className="text-accent size-5 shrink-0" />
        <div className="min-w-0">
          <p className="truncate font-medium">{apercu.titre}</p>
          <p className="text-discret truncate text-xs">{apercu.details}</p>
        </div>
      </div>
      <div>
        <p className="text-3xl font-medium tabular-nums">{apercu.valeur === null ? 'Pas de cours' : montant(apercu.valeur, apercu.devise)}</p>
        {(apercu.gain !== null || apercu.variationPct !== null) && (
          <p className={`mt-1 text-lg tabular-nums ${couleur(sens)}`}>
            {apercu.gain !== null && montant(apercu.gain, apercu.devise, true)}
            {apercu.variationPct !== null && ` (${pourcentage(apercu.variationPct)})`}
            <span className="text-discret text-sm"> {jour}</span>
          </p>
        )}
      </div>
      {apercu.points.length >= 2 ? (
        <div>
          <svg
            width={LARGEUR_CONTENU}
            height={HAUTEUR_COURBE}
            viewBox={`0 0 ${LARGEUR_CONTENU} ${HAUTEUR_COURBE}`}
            className="overflow-visible"
            aria-hidden="true"
          >
            {ligneVeille !== '' && <path d={ligneVeille} fill="none" strokeWidth="1" strokeDasharray="4 3" className="stroke-discret" />}
            <path d={chemin} fill="none" strokeWidth="2" strokeLinejoin="round" className={sens !== null && sens < 0 ? 'stroke-baisse' : 'stroke-hausse'} />
          </svg>
          <p className="text-discret mt-1 flex justify-between text-xs">
            <span>{premier === undefined ? '' : heure(premier.heure)}</span>
            {veille !== null && <span>en pointillé : clôture précédente</span>}
            <span>{dernier === undefined ? '' : heure(dernier.heure)}</span>
          </p>
        </div>
      ) : (
        <p className="text-discret text-sm">{estAujourdhui(apercu.jour) ? 'Pas encore de cours aujourd’hui.' : `Pas de courbe pour la ${jour}.`}</p>
      )}
    </div>
  );
}
