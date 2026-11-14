import { ChevronRight, Folder, Globe, NotebookText, Search, Settings } from 'lucide-react';
import { Fragment, type KeyboardEvent, type MouseEvent, useEffect, useMemo, useRef, useState } from 'react';
import { avertissementRaccourci } from '../lib/raccourci';
import {
  systeme,
  type Apercu as DonneesApercu,
  type ApercuBourse as DonneesApercuBourse,
  type LigneBourse as DonneesLigneBourse,
  type Resultat,
} from '../lib/systeme';
import { Apercu } from './Apercu';
import { ApercuBourse, LigneBourse } from './Bourse';
import {
  type DefinitionFiltre,
  type NomFiltre,
  definition,
  filtreDuSysteme,
  filtreTape,
  filtresProposes,
  poser,
  pourLeSysteme,
  propositions,
  sansMotEnArobase,
} from './filtres';
import { actionTab, ouvreLeDossierChoisi } from './clavier';
import { IconeFiltre } from './icones';
import { Ligne } from './Ligne';
import { ligneVoisine } from './liste';
import { type LigneListe, construireLignes, sectionVoisine } from './sections';
import { adresseTapee } from './web';

/**
 * La barre (spécification §2 à §6) : champ (avec les étiquettes des filtres), liste, ligne d'aide. Cachée
 * sur Échap, sur un clic ailleurs et après un choix, elle se remet à zéro avant de l'être : elle
 * revient vide, sans filtre.
 */
/** Le temps que la barre remise à zéro soit dessinée et sa nouvelle hauteur demandée. */
const DELAI_AVANT_DE_CACHER = 50;
let cacheEnAttente: number | undefined;

/** Ligne d'aide, toujours la même (audit F3) ; seules les erreurs et les avertissements la remplacent. */
const AIDE = '↑↓ choisir · Entrée ouvrir · Tab filtre ou dossier · → entrer · Retour arrière retirer · Échap fermer';

/**
 * Cache la barre un instant plus tard : une fenêtre qui change de taille une fois cachée se rouvre
 * avec une zone pas encore dessinée, un bloc gris visible sur un poste lent (le portable d'Eloi).
 * Un délai plutôt qu'une image (`requestAnimationFrame`) : une fenêtre cachée n'en dessine plus, et
 * la demande attendrait la réouverture pour cacher la barre aussitôt rouverte.
 */
function cacherApresDessin(): void {
  window.clearTimeout(cacheEnAttente);
  cacheEnAttente = window.setTimeout(() => void systeme.cacherBarre(), DELAI_AVANT_DE_CACHER);
}

/** Barre rouverte : une demande de la cacher encore en attente ne vaut plus. */
function annulerCacheEnAttente(): void {
  window.clearTimeout(cacheEnAttente);
  cacheEnAttente = undefined;
}

export function Barre() {
  const [texte, definirTexte] = useState('');
  const [filtres, definirFiltres] = useState<NomFiltre[]>([]);
  const [trouves, definirTrouves] = useState<Resultat[]>([]);
  const [active, definirActive] = useState(0);
  const [erreur, definirErreur] = useState<string | null>(null);
  const [avertissement, definirAvertissement] = useState<string | null>(null);
  // Change à chaque affichage : relit les récents même si le champ était déjà vide.
  const [ouverture, definirOuverture] = useState(0);
  const cadre = useRef<HTMLDivElement>(null);
  const champ = useRef<HTMLInputElement>(null);
  const liste = useRef<HTMLUListElement>(null);
  const derniereRecherche = useRef(0);
  // Dernière position de la souris : une fenêtre qui s'ouvre sous une souris immobile reçoit un
  // mouvement, qui ne doit pas changer la ligne active (spécification §6).
  const derniereSouris = useRef<{ x: number; y: number } | null>(null);
  // Ouverte par Ctrl+K depuis VS Code : Ctrl+O rend Ctrl+K Ctrl+O à VS Code (spécification §2).
  const relais = useRef(false);
  // Dossiers où l'on est entré par → (spécification §6), le dernier est le courant, avec le texte du champ
  // à rendre au retour.
  const [dossiers, definirDossiers] = useState<{ id: string; titre: string; texteAvant: string }[]>([]);
  const dossierCourant = dossiers.at(-1);
  // Aperçu de la ligne active (spécification §6, J11), et ceux déjà lus pendant cette ouverture.
  const [apercu, definirApercu] = useState<{
    id: string;
    donnees: DonneesApercu;
  } | null>(null);
  const apercusLus = useRef(new Map<string, DonneesApercu | null>());
  // @bourse (spécification §4, J10) : liste à part, lue sur l'API bourse, et son aperçu.
  const bourse = filtres.includes('bourse');
  // @web, @google : seulement les pages de Firefox (marque-pages, historique), puis la saisie part sur le Web.
  const moteur = filtres.includes('google') ? 'google' : null;
  const web = filtres.includes('web') || moteur !== null;
  const [lignesBourse, definirLignesBourse] = useState<DonneesLigneBourse[] | null>(null);
  const [apercuBourse, definirApercuBourse] = useState<{ id: string; donnees: DonneesApercuBourse } | null>(null);
  const apercusBourseLus = useRef(new Map<string, DonneesApercuBourse>());
  // Pendant qu'un mot en @ se tape, la liste propose les filtres au lieu des résultats.
  const filtresArobase = propositions(texte, filtres);
  // Le seul mot du champ commence le nom d'un filtre (« vs ») : il est proposé en tête de la liste.
  const proposes = filtresArobase === null && !bourse && dossierCourant === undefined ? filtresProposes(texte, filtres) : [];
  const clefProposes = proposes.join(',');
  // Ligne @bourse active : son aperçu est montré, ou attendu (l'API met jusqu'à 3 s).
  const idBourse = bourse && filtresArobase === null ? lignesBourse?.[active]?.id : undefined;
  const avecApercu = apercu !== null || idBourse !== undefined;

  // Ce que reçoit le Rust : cibles (logiciel, dossier, fichier) et section du genre posé.
  const genre = filtres.find((filtre) => definition(filtre).sorte === 'genre');
  const clefFiltres = filtres.join(',');
  const systemeFiltres = useMemo(() => pourLeSysteme(clefFiltres === '' ? [] : (clefFiltres.split(',') as NomFiltre[])), [clefFiltres]);
  // Récents : une seule section, dans l'ordre où ils ont été ouverts.
  const titreUnique = genre === 'recents' ? 'Récents' : null;

  const saisie = filtresArobase === null ? texte : sansMotEnArobase(texte);
  // Champ vide sans filtre ni dossier : les filtres de genre à choisir. Avec un filtre ou dans un
  // dossier : toutes les lignes. Sinon le meilleur résultat, puis cinq par section et « Plus de… »
  // (spécification §6).
  const champVide = saisie.trim() === '' && dossierCourant === undefined && !bourse && !web;
  const mode = champVide && filtres.length === 0 ? 'filtres' : champVide || genre !== undefined || dossierCourant !== undefined ? 'complet' : 'resume';
  const { lignes, sections } = useMemo(
    () => construireLignes(mode, bourse ? [] : trouves, clefProposes === '' ? [] : (clefProposes.split(',') as NomFiltre[]), titreUnique),
    [mode, trouves, bourse, clefProposes, titreUnique],
  );
  // Adresse tapée (« github.com »), même avec @web : la dernière ligne l'ouvre au lieu de la chercher.
  const site = moteur !== null || bourse ? null : adresseTapee(saisie);
  // Dès qu'un texte est tapé, la dernière ligne le cherche sur le Web (spécification §6).
  const rechercheWeb = filtresArobase === null && !bourse && dossierCourant === undefined && saisie.trim() !== '' ? saisie.trim() : null;
  const nombreDeLignes = filtresArobase?.length ?? (bourse ? (lignesBourse?.length ?? 0) : lignes.length + (rechercheWeb === null ? 0 : 1));
  const ligneCourante: LigneListe | undefined = filtresArobase === null ? lignes[active] : undefined;
  const ligneActive = ligneCourante?.sorte === 'resultat' ? ligneCourante.resultat : undefined;
  const surLeWeb = rechercheWeb !== null && active === lignes.length;

  useEffect(() => {
    void systeme.etatReglages().then((etat) => definirAvertissement(avertissementRaccourci(etat.raccourci, etat.raccourciActif)));
  }, []);

  useEffect(() => {
    const numero = ++derniereRecherche.current;
    if (bourse) {
      // Un instant après la dernière frappe : l'API n'est pas appelée à chaque lettre.
      const attente = window.setTimeout(() => {
        systeme.bourseChercher(saisie).then(
          (lignes) => {
            if (numero === derniereRecherche.current) {
              definirErreur(null);
              definirLignesBourse(lignes);
              definirActive(0);
            }
          },
          (raison: unknown) => {
            if (numero === derniereRecherche.current) {
              definirLignesBourse([]);
              definirErreur(String(raison));
            }
          },
        );
      }, 150);
      return () => window.clearTimeout(attente);
    }
    definirLignesBourse(null);
    if (mode === 'filtres' || (web && saisie.trim() === '')) {
      definirTrouves([]);
      definirActive(0);
      return;
    }
    const recherche = web
      ? systeme.chercher(saisie, [], null, null)
      : systeme.chercher(saisie, systemeFiltres.cibles, dossierCourant?.id ?? null, systemeFiltres.section);
    recherche.then(
      (reponse) => {
        // Une réponse plus ancienne que la dernière frappe est ignorée.
        if (numero === derniereRecherche.current) {
          const trouves = web ? reponse.filter((resultat) => resultat.genre === 'page') : reponse;
          definirTrouves(trouves);
          // Une adresse tapée, ou @web : la ligne qui l'ouvre ou la cherche, après les résultats, est choisie d'office.
          const proposesAlors = clefProposes === '' ? [] : (clefProposes.split(',') as NomFiltre[]);
          const lignesAlors = construireLignes(mode, trouves, proposesAlors, titreUnique).lignes;
          // Un mot qui est aussi un filtre (« vsc ») : Entrée ouvre le logiciel trouvé, Tab pose le filtre.
          const premierResultat = lignesAlors.findIndex((ligne) => ligne.sorte === 'resultat');
          definirActive(web || adresseTapee(saisie) !== null ? lignesAlors.length : Math.max(0, premierResultat));
        }
      },
      (raison: unknown) => definirErreur(String(raison)),
    );
  }, [saisie, systemeFiltres, ouverture, dossierCourant?.id, mode, bourse, web, clefProposes, titreUnique]);

  useEffect(() => {
    definirActive(0);
  }, [filtresArobase?.length]);

  // La fenêtre suit la hauteur du contenu ; sa largeur est fixe (liste et panneau d'aperçu).
  useEffect(() => {
    const element = cadre.current;
    if (element === null) {
      return;
    }
    const observateur = new ResizeObserver(() => {
      void systeme.ajusterTaille(Math.ceil(element.getBoundingClientRect().height));
    });
    observateur.observe(element);
    return () => observateur.disconnect();
  }, []);

  // L'aperçu suit la ligne active, un instant après : passer vite sur les lignes ne lit rien.
  const idActif = ligneActive?.id;
  useEffect(() => {
    if (idActif === undefined) {
      definirApercu(null);
      return;
    }
    const montrer = (donnees: DonneesApercu | null): void => definirApercu(donnees === null ? null : { id: idActif, donnees });
    if (apercusLus.current.has(idActif)) {
      montrer(apercusLus.current.get(idActif) ?? null);
      return;
    }
    let actif = true;
    const attente = window.setTimeout(() => {
      void systeme.apercu(idActif).then(
        (donnees) => {
          apercusLus.current.set(idActif, donnees);
          if (actif) {
            montrer(donnees);
          }
        },
        () => actif && definirApercu(null),
      );
    }, 60);
    return () => {
      actif = false;
      window.clearTimeout(attente);
    };
  }, [idActif]);

  // Aperçu de la ligne @bourse active : courbe et chiffres, gardés pendant cette ouverture.
  useEffect(() => {
    if (idBourse === undefined) {
      definirApercuBourse(null);
      return;
    }
    const connu = apercusBourseLus.current.get(idBourse);
    if (connu !== undefined) {
      definirApercuBourse({ id: idBourse, donnees: connu });
      return;
    }
    let actif = true;
    const attente = window.setTimeout(() => {
      systeme.bourseApercu(idBourse).then(
        (donnees) => {
          apercusBourseLus.current.set(idBourse, donnees);
          if (actif) {
            definirApercuBourse({ id: idBourse, donnees });
          }
        },
        (raison: unknown) => actif && definirErreur(String(raison)),
      );
    }, 100);
    return () => {
      actif = false;
      window.clearTimeout(attente);
    };
  }, [idBourse]);

  useEffect(() => {
    liste.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
  }, [active, lignes, lignesBourse]);

  const remettreAZero = (): void => {
    definirTexte('');
    definirFiltres([]);
    definirDossiers([]);
    definirErreur(null);
    derniereSouris.current = null;
    relais.current = false;
    apercusLus.current.clear();
    definirApercu(null);
    apercusBourseLus.current.clear();
    definirLignesBourse(null);
    definirApercuBourse(null);
  };

  useEffect(() => {
    const auFocus = (): void => {
      annulerCacheEnAttente();
      champ.current?.focus();
      definirOuverture((n) => n + 1);
      // Filtre posé par Ctrl+K depuis Notes seulement (demande d'Eloi), ou par un lancement avec
      // --cible= (la loupe de Notes) ; depuis VS Code, le relais de Ctrl+K Ctrl+O.
      void systeme.contexteOuverture().then((ouverture) => {
        if (ouverture !== null) {
          relais.current = ouverture.relais;
          const demande = filtreDuSysteme(ouverture.cible);
          if (demande !== null) {
            definirFiltres([demande]);
          }
        }
      });
    };
    const auClicAilleurs = (): void => {
      remettreAZero();
      // Déjà cachée (Échap, choix) : la perte du focus vient de là, rien de plus à faire.
      if (!document.hidden) {
        cacherApresDessin();
      }
    };
    window.addEventListener('focus', auFocus);
    window.addEventListener('blur', auClicAilleurs);
    // Premier lancement : la fenêtre a pu prendre le focus avant que la page n'écoute.
    if (document.hasFocus()) {
      auFocus();
    }
    return () => {
      window.removeEventListener('focus', auFocus);
      window.removeEventListener('blur', auClicAilleurs);
    };
  }, []);

  const changerTexte = (valeur: string): void => {
    definirErreur(null);
    const tape = filtreTape(valeur);
    if (tape === null) {
      definirTexte(valeur);
    } else {
      poserFiltre(tape.filtre);
      definirTexte(tape.reste);
    }
  };

  // @bourse, @web, @google ont leur propre liste : ils quittent les dossiers.
  const poserFiltre = (nom: NomFiltre): void => {
    definirFiltres((avant) => poser(avant, nom));
    definirActive(0);
    if (definition(nom).sorte === 'seule') {
      definirDossiers([]);
    }
  };

  const choisirFiltreArobase = (propose: DefinitionFiltre | undefined): void => {
    if (propose !== undefined) {
      poserFiltre(propose.nom);
      definirTexte(sansMotEnArobase(texte));
    }
  };

  // Dernière ligne : ouvre l'adresse tapée, sinon cherche la saisie sur le Web.
  const allerSurLeWeb = (recherche: string): void => {
    (site !== null ? systeme.ouvrirSite(recherche) : systeme.chercherSurLeWeb(recherche, moteur)).then(remettreAZero, (raison: unknown) =>
      definirErreur(String(raison)),
    );
  };

  const choisir = (resultat: Resultat | undefined): void => {
    if (resultat === undefined) {
      return;
    }
    systeme.ouvrir(resultat.id, systemeFiltres.cibles).then(remettreAZero, (raison: unknown) => definirErreur(String(raison)));
  };

  // Curseur du champ au début ou à la fin du texte, sans sélection : les flèches gauche et droite
  // y servent à remonter d'un dossier ou à y entrer.
  const curseurEn = (bout: 'debut' | 'fin'): boolean => {
    const element = champ.current;
    const position = bout === 'debut' ? 0 : texte.length;
    return element !== null && element.selectionStart === position && element.selectionEnd === position;
  };

  const entrerDans = (resultat: Resultat): void => {
    definirDossiers((avant) => [...avant, { id: resultat.id, titre: resultat.titre, texteAvant: texte }]);
    definirTexte('');
  };

  // Entrée sur une ligne : ouvre le résultat, ou pose le filtre (le mot tapé qui le proposait part ;
  // « Plus de… » garde la saisie).
  const valider = (ligne: LigneListe | undefined): void => {
    if (ligne?.sorte === 'resultat') {
      choisir(ligne.resultat);
    } else if (ligne?.sorte === 'filtre') {
      poserFiltre(ligne.filtre);
      definirTexte('');
    } else if (ligne?.sorte === 'plus') {
      poserFiltre(ligne.filtre);
    }
  };

  const remonter = (): void => {
    const quitte = dossiers.at(-1);
    if (quitte !== undefined) {
      definirDossiers((avant) => avant.slice(0, -1));
      definirTexte(quitte.texteAvant);
    }
  };

  const auClavier = (evenement: KeyboardEvent): void => {
    if (relais.current && evenement.ctrlKey && evenement.key.toLowerCase() === 'o') {
      evenement.preventDefault();
      remettreAZero();
      systeme.relayerCtrlKCtrlO().catch((raison: unknown) => definirErreur(String(raison)));
      return;
    }
    switch (evenement.key) {
      case 'Escape':
        evenement.preventDefault();
        remettreAZero();
        cacherApresDessin();
        break;
      case 'ArrowDown':
      case 'ArrowUp':
        evenement.preventDefault();
        definirActive((ligne) => ligneVoisine(ligne, nombreDeLignes, evenement.key === 'ArrowDown' ? 1 : -1));
        break;
      case 'Tab':
        // Tab pose le filtre de la ligne active, sinon le premier proposé, sinon entre dans le dossier
        // actif (il devient une étiquette) ; sans rien de tout cela, section suivante.
        evenement.preventDefault();
        if (filtresArobase !== null) {
          choisirFiltreArobase(filtresArobase[active]);
          break;
        }
        switch (
          actionTab({
            maj: evenement.shiftKey,
            ligneFiltre: ligneCourante?.sorte === 'filtre',
            filtreProposé: proposes[0] !== undefined,
            resultatAvecEnfants: ligneActive?.aDesEnfants === true,
          })
        ) {
          case 'filtre-de-la-ligne':
            valider(ligneCourante);
            break;
          case 'filtre-propose':
            valider({ sorte: 'filtre', filtre: proposes[0]! });
            break;
          case 'entrer-dans-le-dossier':
            entrerDans(ligneActive!);
            break;
          case 'section':
            definirActive((ligne) => sectionVoisine(ligne, sections, evenement.shiftKey ? -1 : 1));
            break;
        }
        break;
      case 'ArrowRight':
        if (ligneActive?.aDesEnfants === true && curseurEn('fin')) {
          evenement.preventDefault();
          entrerDans(ligneActive);
        } else if (ligneCourante !== undefined && ligneCourante.sorte !== 'resultat' && curseurEn('fin')) {
          evenement.preventDefault();
          valider(ligneCourante);
        }
        break;
      case 'ArrowLeft':
        if (dossierCourant !== undefined && curseurEn('debut')) {
          evenement.preventDefault();
          remonter();
        }
        break;
      case 'Enter':
        evenement.preventDefault();
        if (filtresArobase !== null) {
          choisirFiltreArobase(filtresArobase[active]);
        } else if (surLeWeb) {
          allerSurLeWeb(rechercheWeb);
        } else if (
          dossierCourant !== undefined &&
          ouvreLeDossierChoisi({
            dossierEnEtiquette: true,
            champVide: texte.trim() === '',
            logicielPose: filtres.some((filtre) => definition(filtre).sorte === 'logiciel'),
          })
        ) {
          // « vsc », puis un dossier par Tab : Entrée l'ouvre dans VS Code.
          systeme.ouvrir(dossierCourant.id, systemeFiltres.cibles).then(remettreAZero, (raison: unknown) => definirErreur(String(raison)));
        } else {
          valider(ligneCourante);
        }
        break;
      case 'Backspace':
        // Retour arrière dans le champ vide : retire la dernière étiquette, le dossier parcouru, sinon
        // le dernier filtre posé (spécification §4 et §6).
        if (texte === '' && dossierCourant !== undefined) {
          evenement.preventDefault();
          remonter();
        } else if (texte === '' && filtres.length > 0) {
          evenement.preventDefault();
          definirFiltres((avant) => avant.slice(0, -1));
        }
        break;
    }
  };

  const survoler = (rang: number, evenement: MouseEvent): void => {
    const position = { x: evenement.screenX, y: evenement.screenY };
    const avant = derniereSouris.current;
    derniereSouris.current = position;
    if (avant !== null && (avant.x !== position.x || avant.y !== position.y)) {
      definirActive(rang);
    }
  };

  const aide = erreur ?? avertissement;

  return (
    <div ref={cadre} onKeyDown={auClavier} className="bg-fond select-none">
      <div className="border-bordure flex h-14 items-center gap-3 border-b px-4">
        <Search className="text-discret size-5 shrink-0" aria-hidden="true" />
        {filtres.map((pose) => (
          <span
            key={pose}
            className="bg-accent/25 text-accent flex shrink-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-sm font-medium"
            title={definition(pose).description}
          >
            <IconeFiltre nom={pose} taille="size-4" />
            {definition(pose).libelle}
          </span>
        ))}
        {dossiers.length > 0 && (
          <span
            className="bg-texte/10 flex max-w-[60%] min-w-0 shrink items-center gap-1 rounded-md px-2 py-0.5 text-sm"
            title={dossiers.map((d) => d.titre).join(' › ')}
          >
            {dossiers[0]?.id.startsWith('note:') === true ? (
              <NotebookText className="text-accent size-4 shrink-0" aria-hidden="true" />
            ) : (
              <Folder className="text-accent size-4 shrink-0" aria-hidden="true" />
            )}
            {dossiers.map((dossier, rang) => (
              <Fragment key={dossier.id}>
                {rang > 0 && <ChevronRight className="text-discret size-3 shrink-0" aria-hidden="true" />}
                <span className={rang === dossiers.length - 1 ? 'shrink-0' : 'text-discret min-w-0 truncate'}>{dossier.titre}</span>
              </Fragment>
            ))}
          </span>
        )}
        <div className="relative flex min-w-0 flex-1">
          <input
            ref={champ}
            autoFocus
            value={texte}
            onChange={(evenement) => changerTexte(evenement.target.value)}
            placeholder={
              bourse
                ? 'Nom d’une action, ou rien pour le portefeuille'
                : filtres.length === 0
                  ? 'Rechercher'
                  : `Rechercher : ${filtres.map((f) => definition(f).description).join(', ')}`
            }
            aria-label="Rechercher"
            role="combobox"
            aria-expanded={nombreDeLignes > 0}
            aria-controls="resultats"
            spellCheck={false}
            autoComplete="off"
            className="placeholder:text-discret min-w-0 flex-1 bg-transparent text-base outline-0"
          />
        </div>
        <button
          type="button"
          onClick={() => void systeme.ouvrirReglages()}
          title="Réglages"
          aria-label="Réglages"
          className="text-discret hover:bg-texte/10 focus-visible:bg-accent/25 rounded-md p-1.5 outline-0"
        >
          <Settings className="size-4" aria-hidden="true" />
        </button>
      </div>

      {filtresArobase !== null ? (
        <ul ref={liste} id="resultats" role="listbox" aria-label="Filtres" className="py-1">
          {filtresArobase.map((propose, rang) => (
            <LigneFiltre
              key={propose.nom}
              ligne={{ sorte: 'filtre', filtre: propose.nom }}
              propose
              active={rang === active}
              onChoisir={() => choisirFiltreArobase(propose)}
              onSurvoler={(evenement) => survoler(rang, evenement)}
            />
          ))}
        </ul>
      ) : (
        <div className={`flex ${avecApercu ? 'h-[520px]' : ''}`}>
          <div className={`flex w-[640px] shrink-0 flex-col ${avecApercu ? 'h-full' : ''}`}>
            {bourse && (
              <ul
                ref={liste}
                id="resultats"
                role="listbox"
                aria-label="Bourse"
                className={`overflow-y-auto py-1 ${avecApercu ? 'min-h-0 flex-1' : 'max-h-[600px]'}`}
              >
                {lignesBourse?.map((ligne, rang) => (
                  <LigneBourse key={ligne.id} ligne={ligne} active={rang === active} onSurvoler={(evenement) => survoler(rang, evenement)} />
                ))}
                {lignesBourse === null && <li className="text-discret px-4 py-2 text-sm">Lecture de l’API bourse…</li>}
                {lignesBourse?.length === 0 && erreur === null && <li className="text-discret px-4 py-2 text-sm">Aucune action suivie ne correspond</li>}
              </ul>
            )}
            {lignes.length > 0 && (
              <ul
                ref={liste}
                id="resultats"
                role="listbox"
                aria-label={mode === 'filtres' ? 'Filtres' : 'Résultats'}
                className={`overflow-y-auto py-1 ${avecApercu ? 'min-h-0 flex-1' : 'max-h-[600px]'}`}
              >
                {lignes.map((ligne, rang) => {
                  const section = sections.find((s) => s.debut === rang);
                  return (
                    <Fragment key={ligne.sorte === 'resultat' ? ligne.resultat.id : `${ligne.sorte}:${ligne.filtre}`}>
                      {section !== undefined && (
                        <li role="presentation" className="text-discret px-4 pt-2 pb-1 text-xs">
                          {section.titre}
                        </li>
                      )}
                      {ligne.sorte === 'resultat' ? (
                        <Ligne
                          resultat={ligne.resultat}
                          active={rang === active}
                          onChoisir={() => choisir(ligne.resultat)}
                          onSurvoler={(evenement) => survoler(rang, evenement)}
                        />
                      ) : (
                        <LigneFiltre
                          ligne={ligne}
                          propose={mode !== 'filtres'}
                          active={rang === active}
                          onChoisir={() => valider(ligne)}
                          onSurvoler={(evenement) => survoler(rang, evenement)}
                        />
                      )}
                    </Fragment>
                  );
                })}
              </ul>
            )}
            {rechercheWeb !== null && (
              <button
                type="button"
                onClick={() => allerSurLeWeb(rechercheWeb)}
                onMouseMove={(evenement) => survoler(lignes.length, evenement)}
                className={`${surLeWeb ? 'bg-accent/25' : ''} my-1 flex h-10 w-full cursor-default items-center gap-3 px-4 text-left outline-0`}
              >
                <Globe className="text-accent size-5 shrink-0" aria-hidden="true" />
                <span className="min-w-0 truncate">
                  {site !== null ? `Ouvrir ${site}` : `Rechercher « ${rechercheWeb} » sur ${moteur === 'google' ? 'Google' : 'le Web'}`}
                </span>
                <span className="text-discret ml-auto shrink-0 text-xs">{moteur === 'google' ? 'Chrome' : 'Firefox'}</span>
              </button>
            )}
            {mode !== 'filtres' && !bourse && lignes.length === 0 && rechercheWeb === null && (
              <p className="text-discret px-4 py-3 text-sm">
                {dossierCourant !== undefined && saisie.trim() === ''
                  ? dossierCourant.id.startsWith('note:')
                    ? 'Pas de sous-page'
                    : 'Dossier vide'
                  : web
                    ? 'Tapez ce que vous cherchez sur le Web'
                    : saisie.trim() === ''
                      ? 'Rien pour l’instant : tapez pour chercher'
                      : 'Aucun résultat'}
              </p>
            )}
          </div>
          {apercu !== null && ligneActive !== undefined && <Apercu apercu={apercu.donnees} resultat={ligneActive} />}
          {/* Rien à montrer : le panneau reste, vide, pour que la barre garde sa largeur. */}
          {!avecApercu && <div className="border-bordure w-[460px] shrink-0 border-l" />}
          {idBourse !== undefined &&
            (apercuBourse !== null && apercuBourse.id === idBourse ? (
              <ApercuBourse apercu={apercuBourse.donnees} id={apercuBourse.id} />
            ) : (
              <p className="border-bordure text-discret h-full w-[460px] shrink-0 border-l p-4 text-sm">{erreur === null ? 'Lecture de la journée…' : ''}</p>
            ))}
        </div>
      )}

      <p className="border-bordure text-discret border-t px-4 py-2 text-xs">{aide === null ? AIDE : <span className="text-alerte">{aide}</span>}</p>
    </div>
  );
}

/** Ligne d'un filtre à poser (champ vide, ou proposé par le mot tapé), ou « Plus de… » au bout d'une section. */
function LigneFiltre({
  ligne,
  propose,
  active,
  onChoisir,
  onSurvoler,
}: {
  ligne: Exclude<LigneListe, { sorte: 'resultat' }>;
  propose: boolean;
  active: boolean;
  onChoisir: () => void;
  onSurvoler: (evenement: MouseEvent) => void;
}) {
  const plus = ligne.sorte === 'plus';
  const filtre = definition(ligne.filtre);
  return (
    <li
      role="option"
      aria-selected={active}
      onMouseMove={onSurvoler}
      onClick={onChoisir}
      className={`flex cursor-default items-center gap-3 px-4 ${plus ? 'h-8' : 'h-10'} ${active ? 'bg-accent/25' : ''}`}
    >
      {plus ? <span className="size-5 shrink-0" /> : <IconeFiltre nom={ligne.filtre} />}
      <span className={`flex-1 ${plus ? 'text-discret text-sm' : ''}`}>{plus ? `Plus de résultats : ${ligne.section}` : filtre.libelle}</span>
      {!plus && <span className="text-discret text-xs">{filtre.description}</span>}
      {!plus && propose && <kbd className="bg-texte/10 text-discret rounded px-1 font-sans text-xs">Tab</kbd>}
      <ChevronRight className="text-discret size-4 shrink-0" aria-hidden="true" />
    </li>
  );
}
