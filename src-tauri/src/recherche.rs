//! Index des éléments de toutes les sources et commandes de recherche (spécification §3) : chercher,
//! ouvrir, icône. L'interface ne reçoit que des résultats et n'envoie que des choix (spécification §8).

use crate::cibles::{Cible, Cibles};
use crate::sources::applications::{self, Application};
use crate::sources::fichiers::IndexFichiers;
use crate::sources::firefox::{self, Page};
use crate::sources::notes::{self, IconeNote, PageNotes};
use crate::{floue, icones, lancement, recents::Recents};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use tauri::{AppHandle, Manager};

/// Résultats affichés au plus (spécification §6), et par section : une section très fournie (les
/// applications pour une seule lettre) ne doit pas cacher les autres.
pub const MAXIMUM_RESULTATS: usize = 50;
/// Lignes par section quand toutes sont mêlées : la barre en montre 5, une sixième dit qu'il y en a
/// d'autres (« Plus de… »).
const MAXIMUM_PAR_SECTION: usize = 6;

/// Préfixe des identifiants de fichiers et dossiers : `fichier:` suivi du chemin absolu.
const PREFIXE_FICHIER: &str = "fichier:";

/// Préfixe des identifiants de pages de Notes : `note:` suivi de l'UUID de la page.
const PREFIXE_NOTE: &str = "note:";

/// Préfixe des identifiants de pages de Firefox : `page:` suivi de l'adresse.
const PREFIXE_PAGE: &str = "page:";
/// Bonus de score au plus pour la page la plus fréquentée de Firefox, et pour un marque-page :
/// à qualité de correspondance proche, les sites où Eloi va souvent passent devant.
const BONUS_FRECENCE: f64 = 4.0;
const BONUS_MARQUE_PAGE: f64 = 2.0;

/// Ce que la barre sait ouvrir. Les projets viendront au J8.
#[derive(Debug, Clone)]
pub enum Nature {
    Application(Application),
    Page(Page),
    Note(PageNotes),
}

#[derive(Debug, Clone)]
pub struct Element {
    /// Identifiant stable, préfixé par la source (`app:firefox.desktop`) : clé des récents et des icônes.
    pub id: String,
    pub titre: String,
    pub nature: Nature,
    /// Titre et mots-clés normalisés : test rapide avant le calcul du score.
    normalise: String,
    /// Ajouté au score de la recherche floue (fréquentation d'une page).
    bonus: f64,
}

impl Element {
    fn nouveau(id: String, titre: String, nature: Nature, bonus: f64) -> Self {
        let mut normalise = floue::normaliser_en_chaine(&titre);
        let mots_cles: &[String] = match &nature {
            Nature::Application(application) => &application.mots_cles,
            Nature::Page(page) => &page.mots_cles,
            Nature::Note(_) => &[],
        };
        for mot in mots_cles {
            normalise.push(' ');
            normalise.push_str(&floue::normaliser_en_chaine(mot));
        }
        Element {
            id,
            titre,
            nature,
            normalise,
            bonus,
        }
    }

    fn depuis_application(application: Application) -> Self {
        Element::nouveau(
            format!("app:{}", application.id),
            application.nom.clone(),
            Nature::Application(application),
            0.0,
        )
    }

    /// `frecence_max` : la plus forte de toutes les pages, pour ramener le bonus entre 0 et le maximum.
    fn depuis_page(page: Page, frecence_max: i64) -> Self {
        let mut bonus = if frecence_max > 0 {
            BONUS_FRECENCE * (page.frecence as f64).ln_1p() / (frecence_max as f64).ln_1p()
        } else {
            0.0
        };
        if page.marque_page {
            bonus += BONUS_MARQUE_PAGE;
        }
        Element::nouveau(
            format!("{PREFIXE_PAGE}{}", page.url),
            page.titre.clone(),
            Nature::Page(page),
            bonus,
        )
    }

    /// Page de Notes, cherchée par son titre seulement (spécification §3).
    fn depuis_note(page: PageNotes) -> Self {
        Element::nouveau(
            format!("{PREFIXE_NOTE}{}", page.id),
            page.titre.clone(),
            Nature::Note(page),
            0.0,
        )
    }

    fn source(&self) -> String {
        match &self.nature {
            Nature::Application(_) => "Application".into(),
            Nature::Note(page) if page.emplacement.is_empty() => "Notes".into(),
            Nature::Note(page) => format!("Notes · {}", page.emplacement),
            Nature::Page(page) => {
                let sorte = if page.marque_page {
                    "Marque-page"
                } else {
                    "Firefox"
                };
                format!("{sorte} · {}", firefox::hote(&page.url).unwrap_or(""))
            }
        }
    }

    fn groupe(&self) -> &'static str {
        match &self.nature {
            Nature::Application(application) if application.mien => "Logiciels",
            Nature::Application(_) => "Applications",
            Nature::Note(_) => "Notes",
            Nature::Page(page) if page.marque_page => "Marque-pages",
            Nature::Page(_) => "Historique",
        }
    }

    fn genre(&self) -> &'static str {
        match self.nature {
            Nature::Application(_) => "application",
            Nature::Page(_) => "page",
            Nature::Note(_) => "note",
        }
    }

    fn mots_cles(&self) -> &[String] {
        match &self.nature {
            Nature::Application(application) => &application.mots_cles,
            Nature::Page(page) => &page.mots_cles,
            Nature::Note(_) => &[],
        }
    }

    fn a_une_icone(&self) -> bool {
        match &self.nature {
            Nature::Application(application) => application.icone.is_some(),
            Nature::Page(_) => true,
            Nature::Note(page) => matches!(page.icone, Some(IconeNote::Image(_))),
        }
    }

    /// Icône écrite plutôt que demandée : emoji (`emoji:📚`) ou nom Lucide (`lucide:book-open`)
    /// d'une page de Notes.
    fn pictogramme(&self) -> Option<String> {
        match &self.nature {
            Nature::Note(page) => match &page.icone {
                Some(IconeNote::Emoji(emoji)) => Some(format!("emoji:{emoji}")),
                Some(IconeNote::Lucide(nom)) => Some(format!("lucide:{nom}")),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Une ligne de la liste, telle que la page la reçoit.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resultat {
    id: String,
    titre: String,
    /// Source dite à droite de la ligne (spécification §3) : « Application », ou l'emplacement d'un fichier.
    source: String,
    /// `application`, `fichier`, `dossier`, `page` ou `note` : l'icône par défaut en dépend.
    genre: &'static str,
    /// Section de la liste (spécification §6) : Applications, Notes, Dossiers, Fichiers, Marque-pages, Historique.
    groupe: &'static str,
    /// Position (en caractères) des lettres trouvées, mises à la couleur d'accent.
    positions: Vec<usize>,
    /// L'élément a une icône à demander par `icone`.
    a_une_icone: bool,
    /// Icône d'une page de Notes à dessiner par la page : `emoji:…` ou `lucide:…`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pictogramme: Option<String>,
    /// `→` entre dedans (spécification §6) : un dossier, une page de Notes qui a des sous-pages.
    a_des_enfants: bool,
}

impl Resultat {
    fn depuis(element: &Element, positions: Vec<usize>) -> Self {
        Resultat {
            id: element.id.clone(),
            titre: element.titre.clone(),
            source: element.source(),
            genre: element.genre(),
            groupe: element.groupe(),
            positions,
            a_une_icone: element.a_une_icone(),
            pictogramme: element.pictogramme(),
            a_des_enfants: matches!(&element.nature, Nature::Note(page) if page.a_des_enfants),
        }
    }

    fn depuis_fichier(chemin: &Path, dossier: bool, positions: Vec<usize>) -> Self {
        let titre = chemin
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Resultat {
            id: format!("{PREFIXE_FICHIER}{}", chemin.display()),
            titre,
            source: emplacement(chemin.parent().unwrap_or(chemin)),
            genre: if dossier { "dossier" } else { "fichier" },
            groupe: if dossier { "Dossiers" } else { "Fichiers" },
            positions,
            // Icône du thème selon le type (spécification §6).
            a_une_icone: true,
            pictogramme: None,
            a_des_enfants: dossier,
        }
    }
}

/// Dossier montré à droite d'un fichier, avec `~` pour le dossier personnel.
pub fn emplacement(dossier: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(maison) => match dossier.strip_prefix(&maison) {
            Ok(relatif) if relatif.as_os_str().is_empty() => "~".to_string(),
            Ok(relatif) => format!("~/{}", relatif.display()),
            Err(_) => dossier.display().to_string(),
        },
        None => dossier.display().to_string(),
    }
}

/// Dernière lecture de Firefox : refaite seulement quand sa base a changé.
#[derive(Default)]
struct EtatFirefox {
    empreinte_pages: Option<(std::time::SystemTime, Option<std::time::SystemTime>)>,
    pages: Vec<Page>,
    empreinte_icones: Option<(std::time::SystemTime, Option<std::time::SystemTime>)>,
    icones: Option<PathBuf>,
}

pub struct Index {
    elements: RwLock<Vec<Element>>,
    firefox: Mutex<EtatFirefox>,
    /// Absent tant que le premier parcours du dossier personnel n'est pas fini.
    fichiers: RwLock<Option<IndexFichiers>>,
    recents: Mutex<Recents>,
    icones: Mutex<HashMap<String, Option<String>>>,
    rafraichissement: Mutex<()>,
}

impl Index {
    pub fn nouveau() -> Self {
        let fichier = crate::recents::dossier_configuration().map(|d| d.join("recents.json"));
        Index {
            elements: RwLock::new(Vec::new()),
            firefox: Mutex::new(EtatFirefox::default()),
            fichiers: RwLock::new(None),
            recents: Mutex::new(Recents::charger(fichier)),
            icones: Mutex::new(HashMap::new()),
            rafraichissement: Mutex::new(()),
        }
    }

    /// Relit toutes les sources. Un seul rafraîchissement à la fois ; la recherche continue
    /// sur l'ancienne liste pendant ce temps.
    pub fn rafraichir(&self) {
        let Ok(_un_seul) = self.rafraichissement.try_lock() else {
            return;
        };
        let miens = lire_logiciels();
        let mut elements: Vec<Element> = marquer_les_miens(lire_applications(), &miens)
            .into_iter()
            .map(Element::depuis_application)
            .collect();
        let pages = self.lire_firefox();
        let frecence_max = pages.iter().map(|p| p.frecence).max().unwrap_or(0);
        elements.extend(
            pages
                .into_iter()
                .map(|page| Element::depuis_page(page, frecence_max)),
        );
        // Pages de Notes relues à chaque ouverture : quelques centaines de petits fichiers.
        if let Some(coffre) = notes::coffre() {
            elements.extend(
                notes::lire_pages(&coffre)
                    .into_iter()
                    .map(Element::depuis_note),
            );
        }
        if let Ok(mut actuels) = self.elements.write() {
            *actuels = elements;
        }
    }

    /// Pages de Firefox, relues dans une nouvelle copie si sa base a changé depuis la dernière fois.
    /// Copie aussi la base des icônes quand elle a changé.
    fn lire_firefox(&self) -> Vec<Page> {
        let Ok(mut etat) = self.firefox.lock() else {
            return Vec::new();
        };
        let (Some(profil), Some(copies)) = (profil_firefox(), firefox::dossier_copies()) else {
            return Vec::new();
        };
        let empreinte = firefox::empreinte(&profil, "places.sqlite");
        if empreinte.is_some() && empreinte != etat.empreinte_pages {
            if let Some(copie) = firefox::copier(&profil, "places.sqlite", &copies) {
                etat.pages = firefox::lire_pages(&copie);
                etat.empreinte_pages = empreinte;
            }
        }
        let empreinte = firefox::empreinte(&profil, "favicons.sqlite");
        if empreinte.is_some() && empreinte != etat.empreinte_icones {
            if let Some(copie) = firefox::copier(&profil, "favicons.sqlite", &copies) {
                etat.icones = Some(copie);
                etat.empreinte_icones = empreinte;
            }
        }
        etat.pages.clone()
    }

    pub fn remplacer_fichiers(&self, fichiers: IndexFichiers) {
        if let Ok(mut actuel) = self.fichiers.write() {
            *actuel = Some(fichiers);
        }
    }

    pub fn part_de_morts_des_fichiers(&self) -> f64 {
        self.fichiers
            .read()
            .ok()
            .and_then(|f| f.as_ref().map(IndexFichiers::part_de_morts))
            .unwrap_or(0.0)
    }

    /// Relit ces dossiers ; rend les nouveaux dossiers, à surveiller.
    pub fn resynchroniser_fichiers(&self, dossiers: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
        let Ok(mut fichiers) = self.fichiers.write() else {
            return Vec::new();
        };
        let Some(fichiers) = fichiers.as_mut() else {
            return Vec::new();
        };
        dossiers
            .iter()
            .flat_map(|dossier| fichiers.resynchroniser(dossier))
            .collect()
    }

    /// Le fichier ou dossier de cet identifiant, s'il est indexé : chemin, et s'il est un dossier.
    pub fn fichier(&self, id: &str) -> Option<(PathBuf, bool)> {
        let chemin = PathBuf::from(id.strip_prefix(PREFIXE_FICHIER)?);
        let fichiers = self.fichiers.read().ok()?;
        let dossier = fichiers.as_ref()?.est_dossier(&chemin)?;
        Some((chemin, dossier))
    }

    fn resultat_recent(&self, elements: &[Element], id: &str) -> Option<Resultat> {
        if let Some((chemin, dossier)) = self.fichier(id) {
            return Some(Resultat::depuis_fichier(&chemin, dossier, Vec::new()));
        }
        elements
            .iter()
            .find(|e| e.id == id)
            .map(|element| Resultat::depuis(element, Vec::new()))
    }

    /// Résultats pour la saisie, toutes sources mêlées par score, réduits à ce que les cibles savent
    /// ouvrir s'il y en a (spécification §4). Avec `@firefox`, les pages passent devant les fichiers.
    /// Champ vide : les récents encore présents, du plus récent au plus ancien.
    pub fn chercher(&self, saisie: &str, cibles: Cibles) -> Vec<Resultat> {
        let garder = |resultat: &Resultat| {
            cibles.accepte(resultat.genre, &resultat.titre)
                && cibles.dans_la_section(resultat.groupe)
        };
        let Ok(elements) = self.elements.read() else {
            return Vec::new();
        };
        let requete = floue::preparer_saisie(saisie);
        let vide = saisie.trim().is_empty();
        if cibles.section == Some("Récents") || (vide && cibles.section.is_none()) {
            let Ok(recents) = self.recents.lock() else {
                return Vec::new();
            };
            // Les récents, filtrés par la saisie s'il y en a une.
            return recents
                .ids()
                .iter()
                .filter_map(|id| self.resultat_recent(&elements, id))
                .filter(garder)
                .filter_map(|r| {
                    let c = floue::correspondance(&requete, &r.titre)?;
                    Some(Resultat {
                        positions: c.positions,
                        ..r
                    })
                })
                .collect();
        }
        if vide {
            return self.parcourir_section(&elements, &cibles, garder);
        }
        let mots: Vec<String> = floue::normaliser_en_chaine(saisie)
            .split_whitespace()
            .map(String::from)
            .collect();
        let candidats: Vec<&Element> = elements
            .iter()
            .filter(|e| {
                cibles.accepte(e.genre(), &e.titre)
                    && cibles.dans_la_section(e.groupe())
                    && floue::contient_dans_l_ordre(&requete, &e.normalise)
                    && (!matches!(e.nature, Nature::Page(_)) || mots_entiers(&mots, &e.normalise))
            })
            .collect();
        // (score, longueur du titre, rang de la source) : même classement que la recherche floue.
        // Plus de candidats que la limite : le bonus de fréquentation peut encore les reclasser.
        let mut tous: Vec<(f64, usize, usize, Resultat)> = floue::classer_avec_mots_cles(
            saisie,
            &candidats,
            |e| e.titre.as_str(),
            |e| e.mots_cles(),
            MAXIMUM_RESULTATS * 4,
        )
        .into_iter()
        .map(|(element, c)| {
            (
                c.score + element.bonus,
                element.titre.chars().count(),
                0,
                Resultat::depuis(element, c.positions),
            )
        })
        .collect();
        if let Ok(fichiers) = self.fichiers.read() {
            if let Some(fichiers) = fichiers.as_ref() {
                for trouve in fichiers.chercher(&requete, MAXIMUM_RESULTATS) {
                    let resultat =
                        Resultat::depuis_fichier(&trouve.chemin, trouve.dossier, Vec::new());
                    if !garder(&resultat) {
                        continue;
                    }
                    let positions = floue::correspondance(&requete, &trouve.nom)
                        .map(|c| c.positions)
                        .unwrap_or_default();
                    tous.push((
                        trouve.score,
                        trouve.nom.chars().count(),
                        1,
                        Resultat {
                            positions,
                            ..resultat
                        },
                    ));
                }
            }
        }
        tous.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut par_section: HashMap<&'static str, usize> = HashMap::new();
        let mut resultats: Vec<Resultat> = tous
            .into_iter()
            .map(|(_, _, _, r)| r)
            .filter(|r| {
                let compte = par_section.entry(r.groupe).or_default();
                *compte += 1;
                cibles.section.is_some() || *compte <= MAXIMUM_PAR_SECTION
            })
            .take(MAXIMUM_RESULTATS)
            .collect();
        if cibles.logiciel == Some(Cible::Firefox) {
            // Tri stable : chaque groupe garde son ordre.
            resultats.sort_by_key(|r| r.genre != "page");
        }
        resultats
    }

    /// Contenu d'une catégorie au champ vide : applications de A à Z, marque-pages et historique
    /// du plus fréquenté au moins fréquenté, fichiers et dossiers ouverts récemment.
    fn parcourir_section(
        &self,
        elements: &[Element],
        cibles: &Cibles,
        garder: impl Fn(&Resultat) -> bool,
    ) -> Vec<Resultat> {
        match cibles.section {
            // Notes : dans l'ordre du coffre (parents avant leurs sous-pages).
            Some("Notes") => elements
                .iter()
                .filter(|e| cibles.dans_la_section(e.groupe()))
                .map(|e| Resultat::depuis(e, Vec::new()))
                .filter(|r| garder(r))
                .take(MAXIMUM_RESULTATS * 4)
                .collect(),
            Some("Logiciels" | "Applications" | "Marque-pages" | "Historique") => {
                let mut choisis: Vec<&Element> = elements
                    .iter()
                    .filter(|e| cibles.dans_la_section(e.groupe()))
                    .collect();
                choisis.sort_by(|a, b| match (&a.nature, &b.nature) {
                    (Nature::Page(pa), Nature::Page(pb)) => pb.frecence.cmp(&pa.frecence),
                    _ => a.titre.to_lowercase().cmp(&b.titre.to_lowercase()),
                });
                choisis
                    .into_iter()
                    .map(|e| Resultat::depuis(e, Vec::new()))
                    .filter(|r| garder(r))
                    .take(MAXIMUM_RESULTATS * 4)
                    .collect()
            }
            _ => {
                let Ok(recents) = self.recents.lock() else {
                    return Vec::new();
                };
                recents
                    .ids()
                    .iter()
                    .filter_map(|id| self.resultat_recent(elements, id))
                    .filter(|r| garder(r))
                    .collect()
            }
        }
    }

    /// Enfants indexés d'un dossier indexé (spécification §6 : `→` sur un dossier), filtrés par la saisie
    /// et les cibles. Saisie vide : dossiers d'abord, puis par nom.
    pub fn enfants(&self, dossier_id: &str, saisie: &str, cibles: Cibles) -> Vec<Resultat> {
        if let Some(id) = dossier_id.strip_prefix(PREFIXE_NOTE) {
            return self.sous_pages(id, saisie, cibles);
        }
        let Some((dossier, true)) = self.fichier(dossier_id) else {
            return Vec::new();
        };
        let Ok(entrees) = std::fs::read_dir(&dossier) else {
            return Vec::new();
        };
        let requete = floue::preparer_saisie(saisie);
        let mut trouves: Vec<(f64, Resultat)> = entrees
            .flatten()
            .filter_map(|entree| {
                let id = format!("{PREFIXE_FICHIER}{}", entree.path().display());
                let (chemin, est_dossier) = self.fichier(&id)?;
                let resultat = Resultat::depuis_fichier(&chemin, est_dossier, Vec::new());
                if !cibles.accepte(resultat.genre, &resultat.titre)
                    || !cibles.dans_la_section(resultat.groupe)
                {
                    return None;
                }
                let correspondance = floue::correspondance(&requete, &resultat.titre)?;
                Some((
                    correspondance.score,
                    Resultat {
                        positions: correspondance.positions,
                        ..resultat
                    },
                ))
            })
            .collect();
        trouves.sort_by(|(sa, a), (sb, b)| {
            sb.total_cmp(sa)
                .then_with(|| (b.genre == "dossier").cmp(&(a.genre == "dossier")))
                .then_with(|| a.titre.to_lowercase().cmp(&b.titre.to_lowercase()))
        });
        trouves.truncate(MAXIMUM_RESULTATS * 4);
        trouves.into_iter().map(|(_, r)| r).collect()
    }

    /// Sous-pages directes d'une page de Notes (`→` sur la page), filtrées par la saisie et les
    /// cibles. Saisie vide : dans l'ordre de Notes ; sinon par score.
    pub fn sous_pages(&self, id: &str, saisie: &str, cibles: Cibles) -> Vec<Resultat> {
        let Ok(elements) = self.elements.read() else {
            return Vec::new();
        };
        let requete = floue::preparer_saisie(saisie);
        let mut trouvees: Vec<(f64, usize, Resultat)> = elements
            .iter()
            .filter(
                |e| matches!(&e.nature, Nature::Note(page) if page.parent.as_deref() == Some(id)),
            )
            .filter(|e| cibles.accepte(e.genre(), &e.titre))
            .enumerate()
            .filter_map(|(rang, e)| {
                let c = floue::correspondance(&requete, &e.titre)?;
                Some((c.score, rang, Resultat::depuis(e, c.positions)))
            })
            .collect();
        if !saisie.trim().is_empty() {
            trouvees.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        }
        trouvees.into_iter().map(|(_, _, r)| r).collect()
    }

    pub fn element(&self, id: &str) -> Option<Element> {
        self.elements
            .read()
            .ok()?
            .iter()
            .find(|e| e.id == id)
            .cloned()
    }
}

/// Dossiers de données où chercher les applications : `XDG_DATA_HOME` (sinon `~/.local/share`),
/// puis `XDG_DATA_DIRS` de la session (sinon `/usr/local/share:/usr/share`), sans ceux de l'AppImage.
fn dossiers_donnees(environnement: &HashMap<String, String>) -> Vec<PathBuf> {
    let mut dossiers = Vec::new();
    match environnement.get("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(donnees) => dossiers.push(PathBuf::from(donnees)),
        None => {
            if let Some(maison) = environnement.get("HOME") {
                dossiers.push(PathBuf::from(maison).join(".local/share"));
            }
        }
    }
    let systeme = environnement
        .get("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .map_or("/usr/local/share:/usr/share", String::as_str);
    for dossier in systeme.split(':').filter(|d| !d.is_empty()) {
        let dossier = PathBuf::from(dossier);
        if !dossiers.contains(&dossier) {
            dossiers.push(dossier);
        }
    }
    dossiers
}

/// Lanceurs des logiciels d'Eloi (`~/.config/BarreRecherche/logiciels.json`, liste d'identifiants
/// comme `analyse-bancaire.desktop`), propre au poste ; vide sans le fichier (spécification §5).
fn lire_logiciels() -> Vec<String> {
    crate::recents::dossier_configuration()
        .and_then(|dossier| std::fs::read_to_string(dossier.join("logiciels.json")).ok())
        .and_then(|texte| serde_json::from_str::<Vec<String>>(&texte).ok())
        .unwrap_or_default()
}

/// Marque les logiciels d'Eloi ; un des siens dont le programme manque sur ce poste est retiré
/// (le même fichier sert au fixe et au portable).
fn marquer_les_miens(applications: Vec<Application>, miens: &[String]) -> Vec<Application> {
    applications
        .into_iter()
        .filter_map(|mut application| {
            if miens.contains(&application.id) {
                if !application.lancable {
                    return None;
                }
                application.mien = true;
            }
            Some(application)
        })
        .collect()
}

fn lire_applications() -> Vec<Application> {
    let environnement: HashMap<String, String> = lancement::environnement().into_iter().collect();
    let langue = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|nom| environnement.get(*nom).filter(|v| !v.is_empty()));
    let langues = applications::langues(langue.map(String::as_str));
    let bureaux: Vec<String> = environnement
        .get("XDG_CURRENT_DESKTOP")
        .map(|v| v.split(':').map(String::from).collect())
        .unwrap_or_default();
    let chemins: Vec<PathBuf> = environnement
        .get("PATH")
        .map(|v| v.split(':').map(PathBuf::from).collect())
        .unwrap_or_default();
    let contexte = applications::Contexte {
        langues: &langues,
        bureaux: &bureaux,
        chemins: &chemins,
    };
    applications::charger(&dossiers_donnees(&environnement), &contexte)
}

/// Profil de Firefox de la session.
fn profil_firefox() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".mozilla/firefox"))
        .and_then(|d| firefox::profil(&d))
}

/// Peu trouvé, ou `@web` : la saisie part en recherche sur le Web, dans un nouvel onglet de
/// Firefox (demande d'Eloi), avec son moteur par défaut ; avec `@google` (`moteur`), sur Google dans
/// Google Chrome. La barre n'envoie rien elle-même : c'est le navigateur.
#[tauri::command]
pub fn chercher_sur_le_web(texte: String, moteur: Option<String>) -> Result<(), String> {
    let modele = match moteur.as_deref() {
        Some("google") => {
            let modele = firefox::moteur_connu("google").ok_or("Moteur inconnu : google")?;
            return lancement::chercher_sur_google(&texte, modele);
        }
        Some(autre) => return Err(format!("Moteur inconnu : {autre}")),
        None => profil_firefox().and_then(|profil| firefox::moteur_par_defaut(&profil)),
    };
    lancement::chercher_sur_le_web(&texte, modele.as_deref())
}

/// Adresse tapée dans la barre (« github.com ») : ouverte dans un nouvel onglet de Firefox.
#[tauri::command]
pub fn ouvrir_site(adresse: String) -> Result<(), String> {
    lancement::ouvrir_site(&adresse)
}

/// Pages de Firefox : chaque mot tapé doit s'y trouver d'un seul tenant (titre ou site). Des
/// milliers de pages d'historique : des lettres éparpillées en trouvaient trop, sans rapport.
fn mots_entiers(mots: &[String], normalise: &str) -> bool {
    mots.iter().all(|mot| normalise.contains(mot.as_str()))
}

/// Relit les sources dans un fil à part (au démarrage et à chaque ouverture de la barre).
pub fn rafraichir_en_fond(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || app.state::<Index>().rafraichir());
}

#[tauri::command]
pub fn chercher(
    index: tauri::State<'_, Index>,
    saisie: String,
    cibles: Vec<String>,
    dossier: Option<String>,
    section: Option<String>,
) -> Result<Vec<Resultat>, String> {
    let cibles = Cibles::depuis_noms(&cibles)?.dans(section.as_deref())?;
    Ok(match dossier {
        Some(dossier) => index.enfants(&dossier, &saisie, cibles),
        None => index.chercher(&saisie, cibles),
    })
}

/// Ouvre l'élément (dans le logiciel de la cible s'il y en a une), l'inscrit dans les récents et
/// cache la barre.
#[tauri::command]
pub fn ouvrir(
    app: AppHandle,
    index: tauri::State<'_, Index>,
    id: String,
    cibles: Vec<String>,
) -> Result<(), String> {
    let cibles = Cibles::depuis_noms(&cibles)?;
    let refus = || "Cette cible n'ouvre pas ce genre d'élément.".to_string();
    // Seul ce qui est dans l'index s'ouvre : la page ne peut pas faire ouvrir un chemin quelconque.
    if let Some((chemin, dossier)) = index.fichier(&id) {
        let genre = if dossier { "dossier" } else { "fichier" };
        let nom = chemin.file_name().unwrap_or_default().to_string_lossy();
        if !cibles.accepte(genre, &nom) {
            return Err(refus());
        }
        cibles.ouvrir(&chemin, dossier)?;
    } else {
        let element = index.element(&id).ok_or("Cet élément n'existe plus.")?;
        if !cibles.accepte(element.genre(), &element.titre) {
            return Err(refus());
        }
        match &element.nature {
            Nature::Application(application) => {
                lancement::lancer_application(&application.fichier)?
            }
            Nature::Page(page) => lancement::ouvrir_adresse(&page.url)?,
            Nature::Note(page) => ouvrir_dans_notes(&index, &page.id)?,
        }
    }
    if let Ok(mut recents) = index.recents.lock() {
        recents.ajouter(&id);
    }
    if let Some(fenetre) = app.get_webview_window(crate::barre::ETIQUETTE) {
        let _ = fenetre.hide();
    }
    Ok(())
}

/// Lance Notes avec `--page=<uuid>` (spécification §7) : le programme est celui de son lanceur
/// `notes.desktop`, l'identifiant est vérifié comme UUID et passé en argument séparé (règle 1).
/// L'instance de Notes déjà ouverte reçoit l'argument et ouvre la page.
fn ouvrir_dans_notes(index: &Index, id: &str) -> Result<(), String> {
    let argument = notes::argument_page(id).ok_or("Identifiant de page invalide.")?;
    let lanceur = match index.element("app:notes.desktop").map(|e| e.nature) {
        Some(Nature::Application(application)) => application.fichier,
        _ => return Err("Notes introuvable : aucun lanceur notes.desktop.".into()),
    };
    let programme = notes::programme(&lanceur).ok_or("Lanceur de Notes illisible.")?;
    lancement::lancer(&programme, &[std::ffi::OsStr::new(&argument)], None)
}

/// Ce qui donne l'icône d'un identifiant.
enum SourceIcone {
    /// Nom dans le thème ou chemin absolu.
    Theme(String),
    /// Fichier ou dossier : icône du thème selon son type.
    Fichier(PathBuf, bool),
    /// Page de Firefox : icône du site.
    Site(String),
    /// Noms du thème, le premier connu gagne (contenu d'un dossier dans l'aperçu).
    Noms(Vec<String>),
}

impl Index {
    /// Identifiants d'icônes : ceux des résultats, et `cible:<nom>` pour les cibles `@` (icône du
    /// logiciel quand il est installé, sinon du thème).
    fn source_icone(&self, id: &str) -> Option<SourceIcone> {
        if let Some(noms) = id.strip_prefix("theme:") {
            let noms: Vec<String> = noms.split(',').map(String::from).collect();
            return noms
                .iter()
                .all(|n| icones::nom_valide(n))
                .then_some(SourceIcone::Noms(noms));
        }
        if let Some(nom) = id.strip_prefix("cible:") {
            let applications: &[&str] = match nom {
                "vsc" => &[
                    "com.microsoft.VSCode",
                    "code",
                    "com.visualstudio.code",
                    "code-oss",
                    "codium",
                ],
                "terminal" => &["com.mitchellh.ghostty", "ghostty", "org.gnome.Terminal"],
                "firefox" => &["firefox", "firefox-esr", "org.mozilla.firefox"],
                "notes" => &["notes"],
                "google" => &["google-chrome", "com.google.Chrome"],
                "fichier" => return Some(SourceIcone::Theme("text-x-generic".into())),
                "dossier" => return Some(SourceIcone::Theme("folder".into())),
                _ => return None,
            };
            let elements = self.elements.read().ok()?;
            return applications.iter().find_map(|a| {
                let id = format!("app:{a}.desktop");
                elements
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| match &e.nature {
                        Nature::Application(application) => {
                            application.icone.clone().map(SourceIcone::Theme)
                        }
                        Nature::Page(_) | Nature::Note(_) => None,
                    })
            });
        }
        if let Some((chemin, dossier)) = self.fichier(id) {
            return Some(SourceIcone::Fichier(chemin, dossier));
        }
        match self.element(id)?.nature {
            Nature::Application(application) => application.icone.map(SourceIcone::Theme),
            Nature::Page(page) => Some(SourceIcone::Site(page.url)),
            // Image de la page : son chemin absolu, lu comme une icône de fichier.
            Nature::Note(page) => match page.icone {
                Some(IconeNote::Image(chemin)) => {
                    Some(SourceIcone::Theme(chemin.to_string_lossy().into_owned()))
                }
                _ => None,
            },
        }
    }
}

/// Icône de l'élément en adresse `data:`, ou `None`. GTK se consulte sur le fil principal.
#[tauri::command]
pub async fn icone(app: AppHandle, id: String) -> Option<String> {
    let index = app.state::<Index>();
    if let Some(connue) = index.icones.lock().ok()?.get(&id) {
        return connue.clone();
    }
    let adresse = match index.source_icone(&id)? {
        SourceIcone::Site(url) => {
            let base = index.firefox.lock().ok()?.icones.clone()?;
            firefox::icone(&base, &url).map(|(octets, mime)| icones::adresse_octets(&octets, mime))
        }
        source => {
            let (envoi, reception) = std::sync::mpsc::channel();
            app.run_on_main_thread(move || {
                let fichier = match source {
                    SourceIcone::Theme(nom) => icones::fichier(&nom),
                    SourceIcone::Fichier(chemin, dossier) => {
                        icones::fichier_selon_type(&chemin, dossier)
                    }
                    SourceIcone::Noms(noms) => icones::fichier_parmi(&noms),
                    SourceIcone::Site(_) => None,
                };
                let _ = envoi.send(fichier);
            })
            .ok()?;
            reception
                .recv()
                .ok()
                .flatten()
                .and_then(|fichier| icones::adresse_data(&fichier))
        }
    };
    index.icones.lock().ok()?.insert(id, adresse.clone());
    adresse
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_avec(noms: &[&str]) -> Index {
        let index = Index {
            elements: RwLock::new(Vec::new()),
            firefox: Mutex::new(EtatFirefox::default()),
            fichiers: RwLock::new(None),
            recents: Mutex::new(Recents::charger(None)),
            icones: Mutex::new(HashMap::new()),
            rafraichissement: Mutex::new(()),
        };
        *index.elements.write().unwrap() = noms
            .iter()
            .map(|nom| {
                Element::depuis_application(Application {
                    id: format!("{}.desktop", nom.to_lowercase()),
                    nom: nom.to_string(),
                    icone: None,
                    mots_cles: Vec::new(),
                    description: Some(format!("Description de {nom}")),
                    fichier: PathBuf::from("/x"),
                    lancable: true,
                    mien: false,
                })
            })
            .collect();
        index
    }

    #[test]
    fn pages_de_notes_par_leur_titre() {
        let index = index_avec(&["Notes", "Nautilus"]);
        index
            .elements
            .write()
            .unwrap()
            .push(Element::depuis_note(PageNotes {
                id: "01a12cf8-f981-789a-bcf1-d2f5b6b17039".into(),
                titre: "Barre de recherche".into(),
                emplacement: "Correctifs".into(),
                fichier: PathBuf::from("/x/page.md"),
                icone: Some(IconeNote::Emoji("🔍".into())),
                parent: None,
                a_des_enfants: true,
            }));
        let tous = index.chercher("barre", Cibles::default());
        assert_eq!(tous.len(), 1);
        assert_eq!(
            (tous[0].genre, tous[0].groupe, tous[0].source.as_str()),
            ("note", "Notes", "Notes · Correctifs")
        );
        assert_eq!(tous[0].id, "note:01a12cf8-f981-789a-bcf1-d2f5b6b17039");
        assert_eq!(tous[0].pictogramme.as_deref(), Some("emoji:🔍"));
        assert!(!tous[0].a_une_icone);
        assert!(tous[0].a_des_enfants);
        index
            .elements
            .write()
            .unwrap()
            .push(Element::depuis_note(PageNotes {
                id: "01a12cf8-f981-789a-bcf1-d2f5b6b17040".into(),
                titre: "Notes".into(),
                emplacement: "Correctifs › Barre de recherche".into(),
                fichier: PathBuf::from("/x/y/page.md"),
                icone: None,
                parent: Some("01a12cf8-f981-789a-bcf1-d2f5b6b17039".into()),
                a_des_enfants: false,
            }));
        let enfants = index.enfants(
            "note:01a12cf8-f981-789a-bcf1-d2f5b6b17039",
            "",
            Cibles::default(),
        );
        assert_eq!(enfants.len(), 1);
        assert_eq!(enfants[0].titre, "Notes");
        assert!(index
            .enfants(
                "note:01a12cf8-f981-789a-bcf1-d2f5b6b17039",
                "zz",
                Cibles::default()
            )
            .is_empty());
        let avec_notes = Cibles::depuis_noms(&["notes".into()]).unwrap();
        let noms: Vec<String> = index
            .chercher("rech", avec_notes)
            .into_iter()
            .map(|r| r.titre)
            .collect();
        assert_eq!(noms, ["Barre de recherche"]);
        let categorie = Cibles::default().dans(Some("Notes")).unwrap();
        assert_eq!(index.chercher("", categorie).len(), 2);
    }

    #[test]
    fn cherche_et_dit_la_source() {
        let index = index_avec(&["Firefox", "Fichiers", "Terminal"]);
        let resultats = index.chercher("fi", Cibles::default());
        assert_eq!(resultats.len(), 2);
        assert_eq!(resultats[0].source, "Application");
        assert_eq!(resultats[0].positions, [0, 1]);
    }

    #[test]
    fn champ_vide_donne_les_recents_encore_presents() {
        let index = index_avec(&["Firefox", "Fichiers", "Terminal"]);
        assert!(index.chercher("", Cibles::default()).is_empty());
        {
            let mut recents = index.recents.lock().unwrap();
            recents.ajouter("app:disparue.desktop");
            recents.ajouter("app:terminal.desktop");
            recents.ajouter("app:firefox.desktop");
        }
        let titres: Vec<_> = index
            .chercher("  ", Cibles::default())
            .into_iter()
            .map(|r| r.titre)
            .collect();
        assert_eq!(titres, ["Firefox", "Terminal"]);
    }

    #[test]
    fn fichiers_meles_aux_applications_et_dans_les_recents() {
        let racine = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(racine.path().join("Firefox profils")).unwrap();
        std::fs::write(racine.path().join("fiche.txt"), "x").unwrap();
        let index = index_avec(&["Firefox", "Terminal"]);
        index.remplacer_fichiers(IndexFichiers::construire(racine.path()));

        let resultats = index.chercher("fi", Cibles::default());
        let resume: Vec<_> = resultats
            .iter()
            .map(|r| (r.titre.as_str(), r.genre))
            .collect();
        assert_eq!(
            resume,
            [
                ("Firefox", "application"),
                ("fiche.txt", "fichier"),
                ("Firefox profils", "dossier"),
            ]
        );
        assert_eq!(resultats[1].positions, [0, 1]);

        let id = resultats[1].id.clone();
        assert!(index.fichier(&id).is_some());
        assert!(index.fichier("fichier:/etc/passwd").is_none());
        index.recents.lock().unwrap().ajouter(&id);
        assert_eq!(index.chercher("", Cibles::default())[0].titre, "fiche.txt");
        std::fs::remove_file(racine.path().join("fiche.txt")).unwrap();
        index.resynchroniser_fichiers(&BTreeSet::from([racine.path().to_path_buf()]));
        assert!(index.chercher("", Cibles::default()).is_empty());
    }

    #[test]
    fn une_cible_ne_garde_que_ce_qu_elle_ouvre() {
        let racine = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(racine.path().join("Firefox profils")).unwrap();
        std::fs::write(racine.path().join("fiche.txt"), "x").unwrap();
        std::fs::write(racine.path().join("fiche.pdf"), "x").unwrap();
        let index = index_avec(&["Firefox"]);
        index.remplacer_fichiers(IndexFichiers::construire(racine.path()));
        let titres = |noms: &[&str]| -> Vec<String> {
            let noms: Vec<String> = noms.iter().map(|n| n.to_string()).collect();
            index
                .chercher("fi", Cibles::depuis_noms(&noms).unwrap())
                .into_iter()
                .map(|r| r.titre)
                .collect()
        };
        // VS Code et le terminal : des dossiers par défaut, un fichier avec `@fichier`.
        assert_eq!(titres(&["vsc"]), ["Firefox profils"]);
        assert_eq!(titres(&["terminal"]), ["Firefox profils"]);
        assert_eq!(titres(&["vsc", "fichier"]), ["fiche.pdf", "fiche.txt"]);
        assert_eq!(titres(&["firefox"]), ["fiche.pdf"]);
        assert_eq!(titres(&["fichier"]), ["fiche.pdf", "fiche.txt"]);
        assert_eq!(titres(&["dossier"]), ["Firefox profils"]);
        assert_eq!(titres(&["vsc", "dossier"]), ["Firefox profils"]);
        assert_eq!(titres(&["terminal", "fichier"]), ["fiche.pdf", "fiche.txt"]);
        index.recents.lock().unwrap().ajouter("app:firefox.desktop");
        let terminal = Cibles {
            logiciel: Some(Cible::Terminal),
            ..Cibles::default()
        };
        assert!(index.chercher("", terminal).is_empty());
        assert_eq!(index.chercher("", Cibles::default()).len(), 1);
    }

    #[test]
    fn enfants_d_un_dossier_indexe() {
        let racine = tempfile::tempdir().unwrap();
        let r = racine.path();
        std::fs::create_dir_all(r.join("Rapports/Budget")).unwrap();
        std::fs::create_dir_all(r.join("Rapports/.cache")).unwrap();
        std::fs::write(r.join("Rapports/annexe.txt"), "x").unwrap();
        std::fs::write(r.join("Rapports/bilan.pdf"), "x").unwrap();
        let index = index_avec(&[]);
        index.remplacer_fichiers(IndexFichiers::construire(r));
        let id = format!("{PREFIXE_FICHIER}{}", r.join("Rapports").display());
        let titres = |saisie: &str, cibles: Cibles| -> Vec<String> {
            index
                .enfants(&id, saisie, cibles)
                .into_iter()
                .map(|r| r.titre)
                .collect()
        };
        assert_eq!(
            titres("", Cibles::default()),
            ["Budget", "annexe.txt", "bilan.pdf"]
        );
        assert_eq!(titres("bil", Cibles::default()), ["bilan.pdf"]);
        let fichiers = Cibles {
            genre: Some(Cible::Fichier),
            ..Cibles::default()
        };
        assert_eq!(titres("", fichiers), ["annexe.txt", "bilan.pdf"]);
        let dossiers = Cibles::default().dans(Some("Dossiers")).unwrap();
        assert_eq!(titres("", dossiers), ["Budget"]);
        let fichier = format!(
            "{PREFIXE_FICHIER}{}",
            r.join("Rapports/annexe.txt").display()
        );
        assert!(index.enfants(&fichier, "", Cibles::default()).is_empty());
    }

    fn page(titre: &str, url: &str, frecence: i64, marque_page: bool) -> Page {
        Page {
            titre: titre.into(),
            url: url.into(),
            marque_page,
            mots_cles: vec![firefox::hote(url).unwrap().to_string()],
            frecence,
        }
    }

    #[test]
    fn chaque_resultat_dit_sa_section() {
        let racine = tempfile::tempdir().unwrap();
        std::fs::write(racine.path().join("fiche.txt"), "x").unwrap();
        std::fs::create_dir(racine.path().join("Fiches")).unwrap();
        let index = index_avec(&["Firefox"]);
        index.remplacer_fichiers(IndexFichiers::construire(racine.path()));
        index.elements.write().unwrap().extend([
            Element::depuis_page(page("Fil", "https://fil.fr/", 5, true), 5),
            Element::depuis_page(page("Fin", "https://fin.fr/", 5, false), 5),
        ]);
        let mut groupes: Vec<&str> = index
            .chercher("fi", Cibles::default())
            .into_iter()
            .map(|r| r.groupe)
            .collect();
        groupes.sort();
        assert_eq!(
            groupes,
            [
                "Applications",
                "Dossiers",
                "Fichiers",
                "Historique",
                "Marque-pages"
            ]
        );
    }

    #[test]
    fn pages_de_firefox_les_plus_frequentees_devant() {
        let index = index_avec(&["Liferea"]);
        let pages = [
            page(
                "Profil de Jean | LinkedIn",
                "https://www.linkedin.com/in/jean",
                30,
                false,
            ),
            page(
                "Fil d'actualité | LinkedIn",
                "https://www.linkedin.com/feed/",
                21000,
                false,
            ),
            page("Linux Mint", "https://linuxmint.com/", 100, true),
        ];
        index
            .elements
            .write()
            .unwrap()
            .extend(pages.into_iter().map(|p| Element::depuis_page(p, 21000)));
        let resultats = index.chercher("linkedin", Cibles::default());
        let titres: Vec<_> = resultats.iter().map(|r| r.titre.as_str()).collect();
        assert_eq!(
            titres,
            ["Fil d'actualité | LinkedIn", "Profil de Jean | LinkedIn"]
        );
        assert_eq!(resultats[0].source, "Firefox · linkedin.com");
        assert_eq!(resultats[0].genre, "page");
        // Trouvée par son hôte : « mint » est dans « linuxmint.com » comme dans le titre.
        let mint = index.chercher("mint", Cibles::default());
        assert_eq!(mint[0].source, "Marque-page · linuxmint.com");
    }

    #[test]
    #[ignore = "mesure : cargo test --release -- --ignored mesure_vingt_mille_pages --nocapture"]
    fn mesure_vingt_mille_pages() {
        let index = index_avec(&[]);
        let mots = [
            "actualité",
            "LinkedIn",
            "Rust",
            "documentation",
            "recette",
            "course",
            "Mint",
        ];
        index.elements.write().unwrap().extend((0..20_000).map(|i| {
            let titre = format!("{} {} page {i}", mots[i % 7], mots[(i / 7) % 7]);
            Element::depuis_page(
                page(&titre, &format!("https://site{i}.fr/x"), i as i64, false),
                20_000,
            )
        }));
        for saisie in ["l", "lin", "linkedin", "doc rust", "zzz"] {
            let debut = std::time::Instant::now();
            let n = index.chercher(saisie, Cibles::default()).len();
            println!("{saisie:>10} : {n} résultats en {:?}", debut.elapsed());
        }
    }

    #[test]
    fn avec_firefox_les_pages_avant_les_fichiers() {
        let racine = tempfile::tempdir().unwrap();
        std::fs::write(racine.path().join("rust.html"), "x").unwrap();
        let index = index_avec(&[]);
        index.remplacer_fichiers(IndexFichiers::construire(racine.path()));
        index.elements.write().unwrap().push(Element::depuis_page(
            page(
                "Documentation de Rust, le langage",
                "https://doc.rust-lang.org/",
                1,
                false,
            ),
            100,
        ));
        let firefox = Cibles {
            logiciel: Some(Cible::Firefox),
            ..Cibles::default()
        };
        let genres: Vec<_> = index
            .chercher("rust", firefox)
            .into_iter()
            .map(|r| r.genre)
            .collect();
        assert_eq!(genres, ["page", "fichier"]);
        let vsc = Cibles {
            logiciel: Some(Cible::Vsc),
            ..Cibles::default()
        };
        assert_eq!(index.chercher("rust", vsc).len(), 1);
    }

    #[test]
    fn emplacement_avec_tilde() {
        let maison = PathBuf::from(std::env::var("HOME").unwrap());
        assert_eq!(emplacement(&maison), "~");
        assert_eq!(emplacement(&maison.join("Documents/a")), "~/Documents/a");
        assert_eq!(emplacement(Path::new("/etc")), "/etc");
    }

    #[test]
    fn une_section_ne_prend_pas_toute_la_place() {
        let racine = tempfile::tempdir().unwrap();
        for i in 0..30 {
            std::fs::write(racine.path().join(format!("application {i}.txt")), "x").unwrap();
        }
        let noms: Vec<String> = (0..80).map(|i| format!("Application {i}")).collect();
        let refs: Vec<&str> = noms.iter().map(String::as_str).collect();
        let index = index_avec(&refs);
        index.remplacer_fichiers(IndexFichiers::construire(racine.path()));
        let resultats = index.chercher("app", Cibles::default());
        let compte = |resultats: &[Resultat], groupe| {
            resultats.iter().filter(|r| r.groupe == groupe).count()
        };
        assert_eq!(compte(&resultats, "Applications"), MAXIMUM_PAR_SECTION);
        assert_eq!(compte(&resultats, "Fichiers"), MAXIMUM_PAR_SECTION);
        // Dans une catégorie : toutes ses lignes, jusqu'à la limite générale, et rien d'autre.
        let fichiers = index.chercher("app", Cibles::default().dans(Some("Fichiers")).unwrap());
        assert_eq!(compte(&fichiers, "Fichiers"), 30);
        assert_eq!(fichiers.len(), 30);
    }

    #[test]
    fn pages_de_firefox_par_mots_entiers() {
        let index = index_avec(&[]);
        index.elements.write().unwrap().extend([
            Element::depuis_page(
                page(
                    "Documentation de Rust",
                    "https://doc.rust-lang.org/",
                    5,
                    false,
                ),
                50,
            ),
            // « rust » s'y trouve en lettres éparpillées seulement (r-u-s-t) : écartée.
            Element::depuis_page(
                page(
                    "Recettes du sud-ouest, tartes",
                    "https://cuisine.fr/",
                    50,
                    false,
                ),
                50,
            ),
            Element::depuis_page(page("Accueil", "https://github.com/", 5, true), 50),
        ]);
        let titres = |saisie| -> Vec<String> {
            index
                .chercher(saisie, Cibles::default())
                .into_iter()
                .map(|r| r.titre)
                .collect()
        };
        assert_eq!(titres("rust"), ["Documentation de Rust"]);
        assert_eq!(titres("doc rust"), ["Documentation de Rust"]);
        // Le site compte comme le titre.
        assert_eq!(titres("github"), ["Accueil"]);
    }

    #[test]
    fn mes_logiciels_s_ils_sont_sur_le_poste() {
        let application = |id: &str, lancable| Application {
            id: id.into(),
            nom: id.trim_end_matches(".desktop").into(),
            icone: None,
            mots_cles: Vec::new(),
            description: None,
            fichier: PathBuf::from("/x"),
            lancable,
            mien: false,
        };
        let marquees = marquer_les_miens(
            vec![
                application("analyse.desktop", true),
                application("absent.desktop", false),
                application("firefox.desktop", true),
                application("autre.desktop", false),
            ],
            &["analyse.desktop".into(), "absent.desktop".into()],
        );
        let resume: Vec<(&str, bool)> = marquees.iter().map(|a| (a.id.as_str(), a.mien)).collect();
        // Le sien absent du poste disparaît ; une application ordinaire reste, même sans programme trouvé.
        assert_eq!(
            resume,
            [
                ("analyse.desktop", true),
                ("firefox.desktop", false),
                ("autre.desktop", false)
            ]
        );
        let index = index_avec(&[]);
        *index.elements.write().unwrap() = marquees
            .into_iter()
            .map(Element::depuis_application)
            .collect();
        let logiciels = index.chercher("", Cibles::default().dans(Some("Logiciels")).unwrap());
        let titres: Vec<&str> = logiciels.iter().map(|r| r.titre.as_str()).collect();
        assert_eq!(titres, ["analyse"]);
    }

    #[test]
    fn categorie_au_champ_vide() {
        let index = index_avec(&["Zim", "Atom"]);
        index.elements.write().unwrap().extend([
            Element::depuis_page(page("Peu", "https://peu.fr/", 1, false), 50),
            Element::depuis_page(page("Souvent", "https://souvent.fr/", 50, false), 50),
            Element::depuis_page(page("Favori", "https://favori.fr/", 5, true), 50),
        ]);
        let titres = |section| -> Vec<String> {
            index
                .chercher("", Cibles::default().dans(Some(section)).unwrap())
                .into_iter()
                .map(|r| r.titre)
                .collect()
        };
        assert_eq!(titres("Applications"), ["Atom", "Zim"]);
        assert_eq!(titres("Historique"), ["Souvent", "Peu"]);
        assert_eq!(titres("Marque-pages"), ["Favori"]);
        index.recents.lock().unwrap().ajouter("app:zim.desktop");
        assert_eq!(titres("Récents"), ["Zim"]);
        let recents = index.chercher("at", Cibles::default().dans(Some("Récents")).unwrap());
        assert!(recents.is_empty());
    }

    #[test]
    fn dossiers_donnees_de_la_session() {
        let env: HashMap<String, String> = [
            ("HOME", "/home/a"),
            ("XDG_DATA_DIRS", "/usr/share/cinnamon:/usr/share:/usr/share"),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
        assert_eq!(
            dossiers_donnees(&env),
            [
                PathBuf::from("/home/a/.local/share"),
                PathBuf::from("/usr/share/cinnamon"),
                PathBuf::from("/usr/share"),
            ]
        );
    }
}
