import { type MouseEvent } from 'react';
import { type Resultat } from '../lib/systeme';
import { IconeResultat } from './icones';
import { decouper } from './liste';

interface Props {
  resultat: Resultat;
  active: boolean;
  onChoisir: () => void;
  onSurvoler: (evenement: MouseEvent) => void;
}

/** Une ligne : icône, titre aux lettres trouvées en accent, source à droite (spécification §3 et §6). */
export function Ligne({ resultat, active, onChoisir, onSurvoler }: Props) {
  return (
    <li
      role="option"
      aria-selected={active}
      onMouseMove={onSurvoler}
      onClick={onChoisir}
      className={`flex h-10 cursor-default items-center gap-3 px-4 ${active ? 'bg-accent/25' : ''}`}
    >
      <IconeResultat resultat={resultat} />
      <span className="min-w-0 flex-1 truncate">
        {decouper(resultat.titre, resultat.positions).map((morceau, rang) =>
          morceau.trouve ? (
            <span key={rang} className="text-accent">
              {morceau.texte}
            </span>
          ) : (
            <span key={rang}>{morceau.texte}</span>
          ),
        )}
      </span>
      <span className="text-discret max-w-[45%] shrink-0 truncate text-xs" title={resultat.source}>
        {resultat.source}
      </span>
    </li>
  );
}
