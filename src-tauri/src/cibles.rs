//! Cibles `@` (spécification §4) : le logiciel qui ouvre le résultat, à la place du logiciel par défaut.
//! Chaque cible restreint aussi la liste à ce qu'elle sait ouvrir. Lancement sans shell (règle 1) :
//! le chemin est toujours un argument séparé.

use crate::lancement;
use std::ffi::OsStr;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cible {
    /// VS Code (`code`) : ouvre le fichier ou le dossier.
    Vsc,
    /// Ghostty : un terminal dans le dossier (celui du fichier pour un fichier).
    Terminal,
    /// Firefox : ouvre une page HTML ou un PDF.
    Firefox,
    /// Fichiers seulement, ouverts par le logiciel par défaut.
    Fichier,
    /// Dossiers seulement, ouverts dans le gestionnaire de fichiers.
    Dossier,
    /// Pages de Notes seulement, ouvertes dans Notes (spécification §7).
    Notes,
}

/// Extensions que Firefox affiche lui-même (spécification §4).
const EXTENSIONS_FIREFOX: [&str; 4] = ["html", "htm", "xhtml", "pdf"];

impl Cible {
    /// Cible de ce nom, sans le `@` (`vsc`, `terminal`, `firefox`, `fichier`, `dossier`).
    pub fn depuis_nom(nom: &str) -> Option<Self> {
        match nom {
            "vsc" => Some(Cible::Vsc),
            "terminal" => Some(Cible::Terminal),
            "firefox" => Some(Cible::Firefox),
            "fichier" => Some(Cible::Fichier),
            "dossier" => Some(Cible::Dossier),
            "notes" => Some(Cible::Notes),
            _ => None,
        }
    }

    /// Nom de la cible, sans le `@`, tel que la page le connaît.
    pub fn nom(self) -> &'static str {
        match self {
            Cible::Vsc => "vsc",
            Cible::Terminal => "terminal",
            Cible::Firefox => "firefox",
            Cible::Fichier => "fichier",
            Cible::Dossier => "dossier",
            Cible::Notes => "notes",
        }
    }

    /// Ce que la cible sait ouvrir, d'après le genre d'un résultat (`application`, `fichier`,
    /// `dossier`) et son nom.
    pub fn accepte(self, genre: &str, nom: &str) -> bool {
        match self {
            Cible::Vsc | Cible::Terminal => matches!(genre, "fichier" | "dossier"),
            Cible::Firefox if genre == "page" => true,
            Cible::Firefox => {
                genre == "fichier"
                    && Path::new(nom)
                        .extension()
                        .and_then(OsStr::to_str)
                        .is_some_and(|e| {
                            EXTENSIONS_FIREFOX.iter().any(|f| f.eq_ignore_ascii_case(e))
                        })
            }
            Cible::Fichier => genre == "fichier",
            Cible::Dossier => genre == "dossier",
            Cible::Notes => genre == "note",
        }
    }

    /// Un logiciel (`@vsc`, `@terminal`, `@firefox`) ou un genre (`@fichier`, `@dossier`) : une
    /// cible de chaque sorte peut être posée à la fois (spécification §4).
    pub fn est_un_logiciel(self) -> bool {
        matches!(
            self,
            Cible::Vsc | Cible::Terminal | Cible::Firefox | Cible::Notes
        )
    }

    /// Programme et arguments qui ouvrent `chemin` (un dossier si `dossier`). `installe` dit si un
    /// programme est dans le `PATH` : sans Ghostty, `@terminal` prend un autre terminal.
    pub fn commande(
        self,
        chemin: &Path,
        dossier: bool,
        installe: impl Fn(&str) -> bool,
    ) -> (String, Vec<std::ffi::OsString>) {
        match self {
            Cible::Vsc => ("code".into(), vec![chemin.as_os_str().to_owned()]),
            Cible::Terminal => {
                let dossier_de_travail = Self::dossier_de_travail(chemin, dossier);
                let (programme, option) = TERMINAUX
                    .iter()
                    .find(|(programme, _)| installe(programme))
                    .copied()
                    .unwrap_or(TERMINAUX[0]);
                // Sans option connue, le terminal s'ouvre dans son dossier courant (`ouvrir`).
                let arguments = option
                    .map(|option| {
                        let mut argument = std::ffi::OsString::from(option);
                        argument.push(dossier_de_travail.as_os_str());
                        vec![argument]
                    })
                    .unwrap_or_default();
                (programme.into(), arguments)
            }
            Cible::Firefox => ("firefox".into(), vec![chemin.as_os_str().to_owned()]),
            // `@notes` n'accepte aucun fichier : ce cas ne sert pas, il ouvre comme par défaut.
            Cible::Fichier | Cible::Dossier | Cible::Notes => (
                "gio".into(),
                vec!["open".into(), chemin.as_os_str().to_owned()],
            ),
        }
    }

    /// Dossier où s'ouvre un terminal : le dossier lui-même, ou celui du fichier.
    fn dossier_de_travail(chemin: &Path, dossier: bool) -> &Path {
        if dossier {
            chemin
        } else {
            chemin.parent().unwrap_or(chemin)
        }
    }

    /// Ouvre `chemin` avec le logiciel de la cible.
    pub fn ouvrir(self, chemin: &Path, dossier: bool) -> Result<(), String> {
        let (programme, arguments) = self.commande(chemin, dossier, lancement::installe);
        let arguments: Vec<&OsStr> = arguments.iter().map(|a| a.as_os_str()).collect();
        let dossier_courant =
            (self == Cible::Terminal).then(|| Self::dossier_de_travail(chemin, dossier));
        lancement::lancer(&programme, &arguments, dossier_courant)
    }
}

/// Terminaux de `@terminal`, essayés dans l'ordre : Ghostty, celui de Cinnamon, puis celui choisi
/// par le système. Chacun avec l'option qui donne son dossier de travail, s'il en a une.
const TERMINAUX: [(&str, Option<&str>); 3] = [
    ("ghostty", Some("--working-directory=")),
    ("gnome-terminal", Some("--working-directory=")),
    ("x-terminal-emulator", None),
];

/// Cibles posées : au plus un logiciel et un genre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cibles {
    pub logiciel: Option<Cible>,
    pub genre: Option<Cible>,
    /// Catégorie choisie au champ vide ou par « Plus de… » (spécification §6) : seulement cette section.
    pub section: Option<&'static str>,
}

/// Catégories de la barre, dans l'ordre où le champ vide les propose.
pub const SECTIONS: [&str; 8] = [
    "Récents",
    "Logiciels",
    "Applications",
    "Notes",
    "Dossiers",
    "Fichiers",
    "Marque-pages",
    "Historique",
];

impl Cibles {
    /// Cibles nommées par la page (`vsc`, `dossier`…), ou erreur si un nom est inconnu ou si deux
    /// cibles sont de la même sorte.
    pub fn depuis_noms(noms: &[String]) -> Result<Self, String> {
        let mut cibles = Cibles::default();
        for nom in noms {
            let cible = Cible::depuis_nom(nom).ok_or(format!("Cible inconnue : @{nom}"))?;
            let place = if cible.est_un_logiciel() {
                &mut cibles.logiciel
            } else {
                &mut cibles.genre
            };
            if place.replace(cible).is_some() {
                return Err("Deux cibles de la même sorte.".into());
            }
        }
        // VS Code et le terminal ouvrent d'abord des dossiers : sans genre posé, seulement eux
        // (`@fichier` pour un fichier).
        if cibles.genre.is_none() && matches!(cibles.logiciel, Some(Cible::Vsc | Cible::Terminal)) {
            cibles.genre = Some(Cible::Dossier);
        }
        Ok(cibles)
    }

    /// Avec la catégorie nommée par la page, l'une de `SECTIONS`.
    pub fn dans(mut self, section: Option<&str>) -> Result<Self, String> {
        self.section = match section {
            None => None,
            Some(nom) => Some(
                SECTIONS
                    .into_iter()
                    .find(|s| *s == nom)
                    .ok_or(format!("Catégorie inconnue : {nom}"))?,
            ),
        };
        Ok(self)
    }

    /// Le résultat est de la catégorie choisie (« Récents » ne trie pas par section).
    pub fn dans_la_section(&self, groupe: &str) -> bool {
        self.section.is_none_or(|s| s == "Récents" || s == groupe)
    }

    /// Chaque cible posée sait ouvrir ce résultat.
    pub fn accepte(&self, genre: &str, nom: &str) -> bool {
        [self.logiciel, self.genre]
            .into_iter()
            .flatten()
            .all(|c| c.accepte(genre, nom))
    }

    /// Ouvre `chemin` avec le logiciel posé, sinon avec le logiciel par défaut.
    pub fn ouvrir(&self, chemin: &Path, dossier: bool) -> Result<(), String> {
        match self.logiciel {
            Some(logiciel) => logiciel.ouvrir(chemin, dossier),
            None => lancement::ouvrir_fichier(chemin),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn noms_des_cibles() {
        assert_eq!(Cible::depuis_nom("vsc"), Some(Cible::Vsc));
        assert_eq!(Cible::depuis_nom("fichier"), Some(Cible::Fichier));
        assert_eq!(Cible::depuis_nom("dossier"), Some(Cible::Dossier));
        assert_eq!(Cible::depuis_nom("fichiers"), None);
        assert_eq!(Cible::depuis_nom("@vsc"), None);
        assert_eq!(Cible::depuis_nom("notes"), Some(Cible::Notes));
        assert_eq!(Cible::depuis_nom("note"), None);
        for cible in [
            Cible::Vsc,
            Cible::Terminal,
            Cible::Firefox,
            Cible::Fichier,
            Cible::Dossier,
            Cible::Notes,
        ] {
            assert_eq!(Cible::depuis_nom(cible.nom()), Some(cible));
        }
    }

    #[test]
    fn chaque_cible_ne_garde_que_ce_qu_elle_ouvre() {
        assert!(Cible::Vsc.accepte("dossier", "a") && Cible::Vsc.accepte("fichier", "a.txt"));
        assert!(!Cible::Vsc.accepte("application", "Firefox"));
        assert!(Cible::Terminal.accepte("dossier", "a"));
        assert!(Cible::Notes.accepte("note", "ESGI") && !Cible::Notes.accepte("dossier", "a"));
        assert!(!Cible::Vsc.accepte("note", "ESGI") && !Cible::Fichier.accepte("note", "ESGI"));
        assert!(
            Cible::Fichier.accepte("fichier", "a.txt") && !Cible::Fichier.accepte("dossier", "a")
        );
        assert!(
            Cible::Dossier.accepte("dossier", "a") && !Cible::Dossier.accepte("fichier", "a.txt")
        );
    }

    #[test]
    fn firefox_ne_garde_que_html_et_pdf() {
        for nom in ["page.html", "page.HTM", "a.xhtml", "Relevé.PDF"] {
            assert!(Cible::Firefox.accepte("fichier", nom), "{nom}");
        }
        for nom in ["notes.txt", "pdf", "image.png", ".html"] {
            assert!(!Cible::Firefox.accepte("fichier", nom), "{nom}");
        }
        assert!(!Cible::Firefox.accepte("dossier", "site.html"));
    }

    #[test]
    fn un_logiciel_et_un_genre_a_la_fois() {
        let noms = |n: &[&str]| n.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let cibles = Cibles::depuis_noms(&noms(&["vsc", "dossier"])).unwrap();
        assert_eq!(
            cibles,
            Cibles {
                logiciel: Some(Cible::Vsc),
                genre: Some(Cible::Dossier),
                section: None,
            }
        );
        assert!(cibles.accepte("dossier", "Rapports") && !cibles.accepte("fichier", "a.txt"));
        assert!(Cibles::depuis_noms(&noms(&["vsc", "terminal"])).is_err());
        assert!(Cibles::depuis_noms(&noms(&["fichier", "dossier"])).is_err());
        assert!(Cibles::depuis_noms(&noms(&["inconnue"])).is_err());
        let firefox = Cibles::depuis_noms(&noms(&["firefox"])).unwrap();
        assert!(firefox.accepte("page", "LinkedIn") && !firefox.accepte("application", "x"));
        assert!(Cibles::default().accepte("application", "x"));
        let fichiers = Cibles::default().dans(Some("Fichiers")).unwrap();
        assert!(fichiers.dans_la_section("Fichiers") && !fichiers.dans_la_section("Historique"));
        assert!(Cibles::default()
            .dans(Some("Récents"))
            .unwrap()
            .dans_la_section("Applications"));
        assert!(Cibles::default().dans(Some("Tout")).is_err());
    }

    #[test]
    fn terminal_dans_le_dossier_du_fichier() {
        let tous = |_: &str| true;
        let (programme, arguments) =
            Cible::Terminal.commande(Path::new("/home/a/Doc/rapport.pdf"), false, tous);
        assert_eq!(programme, "ghostty");
        assert_eq!(arguments, ["--working-directory=/home/a/Doc"]);
        let (_, arguments) = Cible::Terminal.commande(Path::new("/home/a/Doc"), true, tous);
        assert_eq!(arguments, ["--working-directory=/home/a/Doc"]);
    }

    #[test]
    fn sans_ghostty_un_autre_terminal() {
        let chemin = Path::new("/home/a/Doc");
        let (programme, arguments) =
            Cible::Terminal.commande(chemin, true, |p| p == "gnome-terminal");
        assert_eq!(programme, "gnome-terminal");
        assert_eq!(arguments, ["--working-directory=/home/a/Doc"]);
        // Le terminal du système n'a pas d'option connue : il s'ouvre dans son dossier courant.
        let (programme, arguments) =
            Cible::Terminal.commande(chemin, true, |p| p == "x-terminal-emulator");
        assert_eq!(programme, "x-terminal-emulator");
        assert!(arguments.is_empty());
        // Aucun : Ghostty, pour que l'erreur dise lequel manque.
        assert_eq!(
            Cible::Terminal.commande(chemin, true, |_| false).0,
            "ghostty"
        );
    }

    #[test]
    fn chemin_en_argument_unique() {
        let chemin = PathBuf::from("/home/a/x; rm -rf ~/y $(id).txt");
        let (programme, arguments) = Cible::Vsc.commande(&chemin, false, |_| true);
        assert_eq!(programme, "code");
        assert_eq!(arguments, [chemin.as_os_str()]);
        let (programme, arguments) = Cible::Dossier.commande(&chemin, true, |_| true);
        assert_eq!(programme, "gio");
        assert_eq!(arguments, [OsStr::new("open"), chemin.as_os_str()]);
    }
}
