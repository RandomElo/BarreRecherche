import { Check, Power } from 'lucide-react';
import { useEffect, useState } from 'react';
import { avertissementRaccourci } from '../lib/raccourci';
import { systeme, type EtatReglages } from '../lib/systeme';
import { libelleVersion, versionApplication } from '../version';

/** Fenêtre des réglages (spécification §2) : raccourci, démarrage avec la session, version, Quitter. */
export function Reglages() {
  const [etat, definirEtat] = useState<EtatReglages | null>(null);
  const [erreur, definirErreur] = useState<string | null>(null);

  useEffect(() => {
    void systeme.etatReglages().then(definirEtat);
  }, []);

  const basculer = (actif: boolean): void => {
    definirErreur(null);
    systeme.definirDemarrageAuto(actif).then(definirEtat, (raison: unknown) => definirErreur(String(raison)));
  };

  const avertissement = etat === null ? null : avertissementRaccourci(etat.raccourci, etat.raccourciActif);

  return (
    <main className="flex h-full flex-col gap-5 p-6 text-sm">
      <h1 className="text-lg font-semibold">Réglages</h1>

      {etat !== null && (
        <>
          <p role="status">
            {avertissement ?? (
              <>
                <span className="font-medium">{etat.raccourci}</span> ouvre la barre de n'importe où. Le programme reste en fond pour qu'elle s'ouvre aussitôt.
              </>
            )}
          </p>

          {etat.demarrageAuto === null ? (
            <p className="text-discret">Le démarrage avec la session se règle dans l'AppImage. En développement, il n'y a rien à régler.</p>
          ) : (
            <label className="flex cursor-pointer items-start gap-3">
              {/* Case dessinée ici : la case native suit le thème GTK clair que l'AppImage impose. */}
              <span className="relative mt-0.5 size-4 shrink-0">
                <input
                  type="checkbox"
                  checked={etat.demarrageAuto}
                  onChange={(evenement) => basculer(evenement.target.checked)}
                  className="peer bg-texte/15 hover:bg-texte/25 checked:bg-accent focus-visible:bg-accent/60 checked:focus-visible:bg-accent block size-4 cursor-pointer appearance-none rounded outline-0"
                />
                <Check className="text-fond pointer-events-none absolute inset-0 hidden size-4 p-0.5 peer-checked:block" strokeWidth={3} aria-hidden="true" />
              </span>
              <span>
                Démarrer avec la session
                <span className="text-discret block text-xs">Sans fenêtre : la barre est prête dès l'ouverture de session.</span>
              </span>
            </label>
          )}
          {erreur !== null && (
            <p role="alert" className="text-alerte">
              {erreur}
            </p>
          )}
        </>
      )}

      <div className="mt-auto flex items-center justify-between gap-3">
        <p className="text-discret text-xs">{libelleVersion(versionApplication)}</p>
        <button
          type="button"
          onClick={() => void systeme.quitter()}
          className="bg-texte/10 hover:bg-texte/20 focus-visible:bg-accent/40 inline-flex h-8 items-center gap-2 rounded-md px-3 font-medium outline-0"
        >
          <Power className="size-4" aria-hidden="true" />
          Quitter la barre
        </button>
      </div>
    </main>
  );
}
