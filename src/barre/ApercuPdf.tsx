import { useEffect, useRef, useState } from 'react';
import { systeme } from '../lib/systeme';

/**
 * Première page d'un PDF, dessinée par pdf.js (spécification §6, J11). pdf.js n'est chargé qu'au premier
 * PDF montré. Sans `eval` (CSP stricte) ni scripts du document.
 */
export function ApercuPdf({ id, largeur }: { id: string; largeur: number }) {
  const toile = useRef<HTMLCanvasElement>(null);
  const [etat, definirEtat] = useState<'chargement' | 'pret' | 'erreur'>('chargement');

  useEffect(() => {
    let annule = false;
    let detruire: (() => void) | null = null;
    definirEtat('chargement');
    void (async () => {
      try {
        const [pdfjs, travailleur, octets] = await Promise.all([
          import('pdfjs-dist'),
          import('pdfjs-dist/build/pdf.worker.min.mjs?url'),
          systeme.apercuPdf(id),
        ]);
        pdfjs.GlobalWorkerOptions.workerSrc = travailleur.default;
        const tache = pdfjs.getDocument({ data: new Uint8Array(octets), enableXfa: false });
        detruire = () => void tache.destroy();
        const document = await tache.promise;
        const page = await document.getPage(1);
        const element = toile.current;
        if (annule || element === null) {
          return;
        }
        const densite = window.devicePixelRatio || 1;
        const vue = page.getViewport({ scale: (largeur / page.getViewport({ scale: 1 }).width) * densite });
        element.width = Math.floor(vue.width);
        element.height = Math.floor(vue.height);
        element.style.width = `${largeur}px`;
        await page.render({ canvas: element, viewport: vue }).promise;
        if (!annule) {
          definirEtat('pret');
        }
      } catch {
        if (!annule) {
          definirEtat('erreur');
        }
      }
    })();
    return () => {
      annule = true;
      detruire?.();
    };
  }, [id, largeur]);

  return (
    <div className="relative">
      <canvas ref={toile} className={`rounded-sm bg-white ${etat === 'pret' ? '' : 'hidden'}`} />
      {etat === 'chargement' && <p className="text-discret text-xs">Lecture du PDF…</p>}
      {etat === 'erreur' && <p className="text-discret text-xs">Ce PDF ne s'affiche pas.</p>}
    </div>
  );
}
