import { AppWindow, Bookmark, ChevronRight, Clock, File, Folder, Globe, History, type LucideIcon, NotebookText, Rocket, Search, Settings } from 'lucide-react';
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
  type DefinitionCible,
  type NomCible,
  actionDe,
  categorieTapee,
  cibleTapee,
  completion,
  definition,
  poser,
  propositions,
  sansMotEnArobase,
} from './cibles';
import { IconeCible } from './icones';
import { Ligne } from './Ligne';
import { ligneVoisine } from './liste';
import { type Categorie, type LigneListe, construireLignes, sectionVoisine } from './sections';
import { adresseTapee } from './web';

/** Icône de chaque catégorie, dans la liste et dans l'étiquette. */
const ICONES_CATEGORIES: Record<Categorie, LucideIcon> = {
  Récents: Clock,
  Logiciels: Rocket,
  Applications: AppWindow,
  Notes: NotebookText,
  Dossiers: Folder,
  Fichiers: File,
  'Marque-pages': Bookmark,
  Historique: History,
};

/**
 * La barre (spécification §2 à §6) : champ (avec les étiquettes des cibles), liste, ligne d'aide. Cachée
 * sur Échap, sur un clic ailleurs et après un choix, elle se remet à zéro avant de l'être : elle
 * revient vide, sans cible.
 */
/** Le temps que la barre remise à zéro soit dessinée et sa nouvelle hauteur demandée. */
const DELAI_AVANT_DE_CACHER = 50;
let cacheEnAttente: number | undefined;

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

/** Moins de résultats que cela sur le poste : « Rechercher sur le Web » en dernière ligne. */
const PEU_DE_RESULTATS = 5;

export function Barre() {
  const [texte, definirTexte] = useState('');
  const [cibles, definirCibles] = useState<NomCible[]>([]);
  const [trouves, definirTrouves] = useState<Resultat[]>([]);
  // Saisie à laquelle répondent `trouves` : la recherche sur le Web n'est proposée qu'une fois la réponse arrivée.
  const [saisieDesTrouves, definirSaisieDesTrouves] = useState<string | null>(null);
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
  // Catégorie choisie au champ vide ou par « Plus de… » (spécification §6) : seulement ce type de résultat.
  const [categorie, definirCategorie] = useState<Categorie | null>(null);
  // Aperçu de la ligne active (spécification §6, J11), et ceux déjà lus pendant cette ouverture.
  const [apercu, definirApercu] = useState<{
    id: string;
    donnees: DonneesApercu;
  } | null>(null);
  const apercusLus = useRef(new Map<string, DonneesApercu | null>());
  // @bourse (spécification §4, J10) : liste à part, lue sur l'API bourse, et son aperçu.
  const bourse = cibles.includes('bourse');
  // @web, @google : rien n'est cherché sur le poste, la saisie part sur le Web.
  const moteur = cibles.includes('google') ? 'google' : null;
  const web = cibles.includes('web') || moteur !== null;
  const [lignesBourse, definirLignesBourse] = useState<DonneesLigneBourse[] | null>(null);
  const [apercuBourse, definirApercuBourse] = useState<{ id: string; donnees: DonneesApercuBourse } | null>(null);
  const apercusBourseLus = useRef(new Map<string, DonneesApercuBourse>());
  // Pendant qu'un mot en @ se tape, la liste propose les cibles au lieu des résultats.
  const ciblesProposees = propositions(texte);
  // Premier mot qui commence un nom de cible ou « logiciels » : la fin en grisé, Tab la prend.
  const completee = ciblesProposees === null && !bourse ? completion(texte, cibles) : null;
  // Ligne @bourse active : son aperçu est montré, ou attendu (l'API met jusqu'à 3 s).
  const idBourse = bourse && ciblesProposees === null ? lignesBourse?.[active]?.id : undefined;
  const avecApercu = apercu !== null || idBourse !== undefined;

  const saisie = ciblesProposees === null ? texte : sansMotEnArobase(texte);
  // Champ vide, sans catégorie ni dossier : les catégories à choisir. Dans une catégorie ou un
  // dossier : toutes les lignes. Sinon cinq par section, puis « Plus de… » (spécification §6).
  const mode =
    saisie.trim() === '' && categorie === null && dossierCourant === undefined && !bourse && !web
      ? 'categories'
      : categorie !== null || dossierCourant !== undefined
        ? 'complet'
        : 'resume';
  const { lignes, sections } = useMemo(() => construireLignes(mode, bourse || web ? [] : trouves), [mode, trouves, bourse, web]);
  // Adresse tapée (« github.com »), même avec @web : la dernière ligne l'ouvre au lieu de la chercher.
  const site = moteur !== null || bourse ? null : adresseTapee(saisie);
  // Peu trouvé sur le poste (ou @web posée) : la saisie peut partir en recherche sur le Web, dans
  // Firefox, par une dernière ligne.
  const rechercheWeb =
    ciblesProposees === null &&
    !bourse &&
    saisie.trim() !== '' &&
    (web || site !== null || (mode === 'resume' && trouves.length < PEU_DE_RESULTATS && saisieDesTrouves === saisie))
      ? saisie.trim()
      : null;
  const nombreDeLignes = ciblesProposees?.length ?? (bourse ? (lignesBourse?.length ?? 0) : lignes.length + (rechercheWeb === null ? 0 : 1));
  const ligneCourante: LigneListe | undefined = ciblesProposees === null ? lignes[active] : undefined;
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
    if (mode === 'categories' || web) {
      definirTrouves([]);
      definirActive(0);
      return;
    }
    systeme.chercher(saisie, cibles, dossierCourant?.id ?? null, categorie).then(
      (trouves) => {
        // Une réponse plus ancienne que la dernière frappe est ignorée.
        if (numero === derniereRecherche.current) {
          definirTrouves(trouves);
          definirSaisieDesTrouves(saisie);
          // Une adresse tapée : la ligne qui l'ouvre, après les résultats, est choisie d'office.
          definirActive(!web && adresseTapee(saisie) !== null ? construireLignes(mode, trouves).lignes.length : 0);
        }
      },
      (raison: unknown) => definirErreur(String(raison)),
    );
  }, [saisie, cibles, ouverture, dossierCourant?.id, categorie, mode, bourse, web]);

  useEffect(() => {
    definirActive(0);
  }, [ciblesProposees?.length]);

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
    definirCibles([]);
    definirDossiers([]);
    definirCategorie(null);
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
      // Aucune cible posée d'office (demande d'Eloi) : le logiciel au premier plan ne sert plus
      // qu'au relais de Ctrl+K Ctrl+O vers VS Code.
      void systeme.contexteOuverture().then((ouverture) => {
        if (ouverture !== null) {
          relais.current = ouverture.relais;
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
    return () => {
      window.removeEventListener('focus', auFocus);
      window.removeEventListener('blur', auClicAilleurs);
    };
  }, []);

  const changerTexte = (valeur: string): void => {
    definirErreur(null);
    // « logiciel » ou « logiciels » en tête du champ, suivi d'une espace : la catégorie de ses logiciels.
    const logiciels = categorieTapee(valeur);
    if (logiciels !== null) {
      // Une catégorie cherche sur le poste : @web, @google ou @bourse partent.
      definirCibles((avant) => avant.filter((cible) => definition(cible).sorte !== 'seule'));
      definirCategorie('Logiciels');
      definirActive(0);
      definirTexte(logiciels);
      return;
    }
    const tapee = cibleTapee(valeur);
    if (tapee === null) {
      definirTexte(valeur);
    } else {
      poserCible(tapee.cible);
      definirTexte(tapee.reste);
    }
  };

  // @bourse a sa propre liste : elle quitte la catégorie et les dossiers.
  const poserCible = (nom: NomCible): void => {
    definirCibles((avant) => poser(avant, nom));
    if (definition(nom).sorte === 'seule') {
      definirCategorie(null);
      definirDossiers([]);
    }
  };

  const choisirCible = (proposee: DefinitionCible | undefined): void => {
    if (proposee !== undefined) {
      poserCible(proposee.nom);
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
    systeme.ouvrir(resultat.id, cibles).then(remettreAZero, (raison: unknown) => definirErreur(String(raison)));
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

  const choisirCategorie = (choisie: Categorie): void => {
    definirCategorie(choisie);
    definirActive(0);
  };

  // Entrée sur une ligne : ouvre le résultat, ou entre dans la catégorie.
  const valider = (ligne: LigneListe | undefined): void => {
    if (ligne?.sorte === 'resultat') {
      choisir(ligne.resultat);
    } else if (ligne !== undefined) {
      choisirCategorie(ligne.categorie);
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
        evenement.preventDefault();
        if (ciblesProposees !== null) {
          choisirCible(ciblesProposees[active]);
        } else if (completee !== null && !evenement.shiftKey) {
          changerTexte(completee.texte);
        } else {
          definirActive((ligne) => sectionVoisine(ligne, sections, evenement.shiftKey ? -1 : 1));
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
        if (ciblesProposees !== null) {
          choisirCible(ciblesProposees[active]);
        } else if (surLeWeb) {
          allerSurLeWeb(rechercheWeb);
        } else {
          valider(ligneCourante);
        }
        break;
      case 'Backspace':
        // Retour arrière dans le champ vide : remonte d'un dossier, sinon retire la catégorie, sinon
        // la dernière cible posée (spécification §4 et §6).
        if (texte === '' && dossierCourant !== undefined) {
          evenement.preventDefault();
          remonter();
        } else if (texte === '' && categorie !== null) {
          evenement.preventDefault();
          definirCategorie(null);
        } else if (texte === '' && cibles.length > 0) {
          evenement.preventDefault();
          definirCibles((avant) => avant.slice(0, -1));
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

  const aideParDefaut =
    ciblesProposees !== null
      ? 'Tab ou Entrée pour choisir la cible'
      : completee !== null
        ? `Tab pour filtrer : ${completee.filtre} (sinon « ${texte.trim()} » reste une recherche), Entrée pour ouvrir`
        : surLeWeb
          ? site !== null
            ? 'Entrée pour ouvrir le site dans un onglet de Firefox'
            : `${web ? '' : lignes.length === 0 ? 'Rien trouvé sur le poste : ' : 'Peu trouvé sur le poste : '}Entrée pour chercher sur ${moteur === 'google' ? 'Google dans Chrome' : 'le Web dans Firefox'}`
          : bourse
            ? saisie.trim() === ''
              ? 'Ton portefeuille : flèches pour voir une position, tape le nom d’une action pour son cours, Retour arrière retire @bourse'
              : 'Flèches pour voir la courbe d’une action, champ vide pour le portefeuille'
            : ligneCourante?.sorte === 'categorie'
              ? 'Flèches pour choisir une catégorie, Entrée ou → pour la parcourir, ou tapez pour chercher partout'
              : ligneCourante?.sorte === 'plus'
                ? `Entrée pour voir tous les résultats de ${ligneCourante.categorie}`
                : ligneActive?.aDesEnfants === true
                  ? `Entrée pour ${cibles.length > 0 ? actionDe(cibles) : ligneActive.genre === 'note' ? 'ouvrir dans Notes' : 'ouvrir le dossier'}, → pour voir ${ligneActive.genre === 'note' ? 'ses sous-pages' : 'son contenu'}${dossierCourant !== undefined ? ', ← pour remonter' : ''}`
                  : dossierCourant !== undefined
                    ? 'Entrée pour ouvrir, ← ou Retour arrière pour remonter, Tab pour passer à la section suivante'
                    : categorie !== null
                      ? `Entrée pour ouvrir, Retour arrière pour quitter ${categorie}`
                      : cibles.length > 0
                        ? `Entrée pour ${actionDe(cibles)}${relais.current ? ', Ctrl+O pour ouvrir un dossier' : ''}, Retour arrière retire @${cibles.at(-1) ?? ''}`
                        : 'Flèches pour choisir, Entrée pour ouvrir, Tab pour passer à la section suivante, @ pour choisir le logiciel';
  const aide = erreur ?? avertissement;

  return (
    <div ref={cadre} onKeyDown={auClavier} className="bg-fond select-none">
      <div className="border-bordure flex h-14 items-center gap-3 border-b px-4">
        <Search className="text-discret size-5 shrink-0" aria-hidden="true" />
        {categorie !== null && <EtiquetteCategorie categorie={categorie} />}
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
        {cibles.map((posee) => (
          <span
            key={posee}
            className="bg-accent/25 text-accent flex shrink-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-sm font-medium"
            title={definition(posee).libelle}
          >
            <IconeCible nom={posee} taille="size-4" />@{posee}
          </span>
        ))}
        <div className="relative flex min-w-0 flex-1">
          <input
            ref={champ}
            autoFocus
            value={texte}
            onChange={(evenement) => changerTexte(evenement.target.value)}
            placeholder={
              bourse
                ? 'Nom d’une action, ou rien pour le portefeuille'
                : categorie !== null
                  ? `Rechercher dans ${categorie}`
                  : cibles.length === 0
                    ? 'Rechercher'
                    : `Rechercher : ${cibles.map((c) => definition(c).libelle).join(', ')}`
            }
            aria-label="Rechercher"
            role="combobox"
            aria-expanded={nombreDeLignes > 0}
            aria-controls="resultats"
            spellCheck={false}
            autoComplete="off"
            className="placeholder:text-discret min-w-0 flex-1 bg-transparent text-base outline-0"
          />
          {completee !== null && (
            // Même police que le champ : le texte tapé, invisible, place la suite grisée juste après,
            // puis le filtre que `Tab` posera.
            <span className="pointer-events-none absolute inset-0 flex items-center overflow-hidden text-base whitespace-pre" aria-hidden="true">
              <span className="invisible">{texte}</span>
              <span className="text-discret">{completee.suite}</span>
              <span className="bg-texte/10 text-discret ml-3 flex items-center gap-1.5 rounded-md px-2 py-0.5 text-xs">
                filtre {completee.filtre}
                <kbd className="bg-texte/10 rounded px-1 font-sans">Tab</kbd>
              </span>
            </span>
          )}
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

      {ciblesProposees !== null ? (
        <ul ref={liste} id="resultats" role="listbox" aria-label="Cibles" className="py-1">
          {ciblesProposees.map((proposee, rang) => (
            <li
              key={proposee.nom}
              role="option"
              aria-selected={rang === active}
              onMouseMove={(evenement) => survoler(rang, evenement)}
              onClick={() => choisirCible(proposee)}
              className={`flex h-10 cursor-default items-center gap-3 px-4 ${rang === active ? 'bg-accent/25' : ''}`}
            >
              <IconeCible nom={proposee.nom} />
              <span className="flex-1">@{proposee.nom}</span>
              <span className="text-discret text-xs">{proposee.libelle}</span>
            </li>
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
                aria-label={mode === 'categories' ? 'Catégories' : 'Résultats'}
                className={`overflow-y-auto py-1 ${avecApercu ? 'min-h-0 flex-1' : 'max-h-[600px]'}`}
              >
                {lignes.map((ligne, rang) => {
                  const section = sections.find((s) => s.debut === rang);
                  return (
                    <Fragment key={ligne.sorte === 'resultat' ? ligne.resultat.id : `${ligne.sorte}:${ligne.categorie}`}>
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
                        <LigneCategorie
                          ligne={ligne}
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
            {mode !== 'categories' && !bourse && lignes.length === 0 && rechercheWeb === null && (
              <p className="text-discret px-4 py-3 text-sm">
                {dossierCourant !== undefined && saisie.trim() === ''
                  ? dossierCourant.id.startsWith('note:')
                    ? 'Pas de sous-page'
                    : 'Dossier vide'
                  : web
                    ? 'Tapez ce que vous cherchez sur le Web, ou un filtre pour chercher sur le poste : vsc, dossier, fichier, notes, logiciels…'
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

      <p className="border-bordure text-discret border-t px-4 py-2 text-xs">{aide === null ? aideParDefaut : <span className="text-alerte">{aide}</span>}</p>
    </div>
  );
}

/** Étiquette de la catégorie choisie, au début du champ. */
function EtiquetteCategorie({ categorie }: { categorie: Categorie }) {
  const Icone = ICONES_CATEGORIES[categorie];
  return (
    <span className="bg-texte/10 flex shrink-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-sm font-medium">
      <Icone className="text-accent size-4" aria-hidden="true" />
      {categorie}
    </span>
  );
}

/** Ligne d'une catégorie à parcourir, ou « Plus de… » au bout d'une section. */
function LigneCategorie({
  ligne,
  active,
  onChoisir,
  onSurvoler,
}: {
  ligne: Exclude<LigneListe, { sorte: 'resultat' }>;
  active: boolean;
  onChoisir: () => void;
  onSurvoler: (evenement: MouseEvent) => void;
}) {
  const Icone = ICONES_CATEGORIES[ligne.categorie];
  const plus = ligne.sorte === 'plus';
  return (
    <li
      role="option"
      aria-selected={active}
      onMouseMove={onSurvoler}
      onClick={onChoisir}
      className={`flex cursor-default items-center gap-3 px-4 ${plus ? 'h-8' : 'h-10'} ${active ? 'bg-accent/25' : ''}`}
    >
      {plus ? <span className="size-5 shrink-0" /> : <Icone className="text-accent size-5 shrink-0" aria-hidden="true" />}
      <span className={`flex-1 ${plus ? 'text-discret text-sm' : ''}`}>{plus ? `Plus de résultats : ${ligne.categorie}` : ligne.categorie}</span>
      <ChevronRight className="text-discret size-4 shrink-0" aria-hidden="true" />
    </li>
  );
}
