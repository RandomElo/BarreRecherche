//! Lancement des logiciels (règle 1) : programme et arguments séparés, jamais par un shell. Le
//! logiciel lancé est détaché de la barre et reçoit l'environnement de la session, débarrassé de ce
//! que l'AppImage y a mis pour elle-même (thème GTK forcé, chemins vers ses propres bibliothèques).

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

/// Variables posées par le lanceur de l'AppImage et son crochet GTK, sans sens pour un autre logiciel.
const VARIABLES_DE_L_APPIMAGE: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "APPIMAGE_GTK_THEME",
    "GTK_THEME",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_PATH",
    "GTK_IM_MODULE_FILE",
    "GDK_PIXBUF_MODULE_FILE",
    "GSETTINGS_SCHEMA_DIR",
    "GI_TYPELIB_PATH",
    "GIO_MODULE_DIR",
];

/// Environnement à donner aux logiciels lancés. Hors AppImage (`appdir` absent), rien ne change.
/// Dans l'AppImage : ses variables retirées, et dans les listes de chemins (`a:b:c`) les éléments
/// qui pointent dans l'AppImage retirés. Le crochet GTK ajoute aussi `/usr/share` en tête de
/// `XDG_DATA_DIRS` : retiré s'il figure plus loin, pour retrouver l'ordre de la session.
pub fn environnement_propre(
    variables: impl IntoIterator<Item = (String, String)>,
    appdir: Option<&str>,
) -> Vec<(String, String)> {
    let Some(appdir) = appdir
        .map(|a| a.trim_end_matches('/'))
        .filter(|a| !a.is_empty())
    else {
        return variables.into_iter().collect();
    };
    let dans_l_appimage =
        |valeur: &str| valeur == appdir || valeur.starts_with(&format!("{appdir}/"));
    variables
        .into_iter()
        .filter(|(nom, _)| !VARIABLES_DE_L_APPIMAGE.contains(&nom.as_str()))
        .filter_map(|(nom, valeur)| {
            if !valeur.contains(appdir) {
                return Some((nom, valeur));
            }
            let mut elements: Vec<&str> =
                valeur.split(':').filter(|e| !dans_l_appimage(e)).collect();
            if nom == "XDG_DATA_DIRS"
                && elements.first() == Some(&"/usr/share")
                && elements[1..].contains(&"/usr/share")
            {
                elements.remove(0);
            }
            let propre = elements.join(":");
            (!propre.is_empty()).then_some((nom, propre))
        })
        .collect()
}

/// Environnement propre du processus courant.
pub fn environnement() -> Vec<(String, String)> {
    let appdir = std::env::var("APPDIR").ok();
    environnement_propre(std::env::vars(), appdir.as_deref())
}

/// Lance `programme` avec ses arguments, détaché (son propre groupe de processus, sans entrée ni
/// sortie), dans `dossier` s'il est donné. Le processus est attendu dans un fil pour ne pas laisser
/// de zombie.
pub fn lancer(programme: &str, arguments: &[&OsStr], dossier: Option<&Path>) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let mut commande = Command::new(programme);
    commande
        .args(arguments)
        .env_clear()
        .envs(environnement())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if let Some(dossier) = dossier {
        commande.current_dir(dossier);
    } else if let Some(maison) = std::env::var_os("HOME") {
        commande.current_dir(maison);
    }
    let mut enfant = commande
        .spawn()
        .map_err(|e| format!("{programme} n'a pas pu être lancé : {e}"))?;
    std::thread::spawn(move || {
        let _ = enfant.wait();
    });
    Ok(())
}

/// Vrai si `programme` est un exécutable du `PATH` de la session (celui des logiciels lancés).
pub fn installe(programme: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    environnement()
        .into_iter()
        .find(|(nom, _)| nom == "PATH")
        .is_some_and(|(_, chemins)| {
            chemins.split(':').filter(|d| !d.is_empty()).any(|dossier| {
                Path::new(dossier)
                    .join(programme)
                    .metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            })
        })
}

/// Lance une application par son fichier `.desktop` : `gio launch` applique la spécification
/// (codes `%f`, `Terminal=true`, `Path=`, activation D-Bus) comme le menu.
pub fn lancer_application(fichier: &Path) -> Result<(), String> {
    lancer("gio", &[OsStr::new("launch"), fichier.as_os_str()], None)
}

/// Ouvre un fichier avec le logiciel par défaut du système, un dossier dans le gestionnaire de
/// fichiers (`gio open`, le chemin en argument séparé).
pub fn ouvrir_fichier(chemin: &Path) -> Result<(), String> {
    lancer("gio", &[OsStr::new("open"), chemin.as_os_str()], None)
}

/// Longueur au plus d'une recherche sur le Web.
const LONGUEUR_RECHERCHE_WEB: usize = 500;

/// Texte d'une recherche sur le Web, sans les espaces autour, ou `None` s'il est vide, trop long
/// ou contient un caractère de contrôle.
fn texte_de_recherche(texte: &str) -> Option<&str> {
    let texte = texte.trim();
    (!texte.is_empty()
        && texte.chars().count() <= LONGUEUR_RECHERCHE_WEB
        && !texte.chars().any(char::is_control))
    .then_some(texte)
}

/// Arguments de Firefox pour chercher `texte` avec son moteur par défaut (dans une nouvelle
/// fenêtre : `--search` n'ouvre pas d'onglet), ou `None` si le texte est refusé. Un texte qui
/// commencerait par `-` prend une espace devant : Firefox ne doit jamais le lire comme une option.
pub fn arguments_recherche_web(texte: &str) -> Option<[String; 2]> {
    let texte = texte_de_recherche(texte)?;
    let texte = if texte.starts_with('-') {
        format!(" {texte}")
    } else {
        texte.to_string()
    };
    Some(["--search".to_string(), texte])
}

/// Adresse de la recherche : `{searchTerms}` du modèle remplacé par le texte encodé. Le modèle
/// doit être une adresse `http(s)`.
pub fn adresse_de_recherche(modele: &str, texte: &str) -> Option<String> {
    let texte = texte_de_recherche(texte)?;
    let encode: String = texte
        .bytes()
        .map(|octet| {
            if octet.is_ascii_alphanumeric() || b"-._~".contains(&octet) {
                char::from(octet).to_string()
            } else {
                format!("%{octet:02X}")
            }
        })
        .collect();
    (adresse_web(modele) && modele.contains("{searchTerms}"))
        .then(|| modele.replace("{searchTerms}", &encode))
}

/// Cherche `texte` sur le Web dans un nouvel onglet de Firefox, avec le moteur de `modele`
/// (spécification §6) ; sans modèle, son moteur par défaut par `--search`, dans une fenêtre.
pub fn chercher_sur_le_web(texte: &str, modele: Option<&str>) -> Result<(), String> {
    if let Some(modele) = modele {
        let adresse = adresse_de_recherche(modele, texte).ok_or("Recherche refusée.")?;
        return lancer(
            "firefox",
            &[OsStr::new("--new-tab"), OsStr::new(&adresse)],
            None,
        );
    }
    let arguments = arguments_recherche_web(texte).ok_or("Recherche refusée.")?;
    let arguments: Vec<&OsStr> = arguments.iter().map(OsStr::new).collect();
    lancer("firefox", &arguments, None)
}

/// Cherche `texte` sur Google dans Google Chrome (`@google`, demande d'Eloi), dans un nouvel
/// onglet de sa fenêtre ouverte ; sans Chrome sur le poste, dans un onglet de Firefox.
pub fn chercher_sur_google(texte: &str, modele: &str) -> Result<(), String> {
    if !installe("google-chrome") {
        return chercher_sur_le_web(texte, Some(modele));
    }
    let adresse = adresse_de_recherche(modele, texte).ok_or("Recherche refusée.")?;
    lancer("google-chrome", &[OsStr::new(&adresse)], None)
}

/// Domaines de premier niveau qui font d'un texte sans `http` une adresse (« github.com ») :
/// une liste courte, pour qu'un nom de fichier (« rapport.pdf ») n'en soit pas une.
const DOMAINES_RECONNUS: &[&str] = &[
    "com", "fr", "org", "net", "io", "dev", "app", "eu", "be", "ch", "ca", "de", "uk", "co",
    "info", "me", "ai", "gg", "tv", "xyz", "edu", "gov",
];

/// Adresse tapée dans la barre (« github.com/x », « https://… ») : l'adresse à ouvrir, avec
/// `https://` devant s'il n'y en a pas, ou `None` si le texte n'en a pas la forme. Même règle que
/// `adresseTapee` de l'interface.
pub fn adresse_tapee(texte: &str) -> Option<String> {
    let texte = texte.trim();
    if texte.is_empty()
        || texte.len() > 2000
        || texte.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let minuscule = texte.to_lowercase();
    if minuscule.starts_with("https://") || minuscule.starts_with("http://") {
        return (!texte.split_once("://")?.1.is_empty()).then(|| texte.to_string());
    }
    let hote = minuscule.split('/').next()?;
    let hote = match hote.split_once(':') {
        Some((nom, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => nom,
        Some(_) => return None,
        None => hote,
    };
    let etiquettes: Vec<&str> = hote.split('.').collect();
    let valides = etiquettes.len() >= 2
        && etiquettes
            .iter()
            .all(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    let reconnu = etiquettes.first() == Some(&"www")
        || etiquettes
            .last()
            .is_some_and(|d| DOMAINES_RECONNUS.contains(d));
    (valides && reconnu).then(|| format!("https://{texte}"))
}

/// Ouvre dans un nouvel onglet de Firefox une adresse tapée dans la barre (`adresse_tapee`).
pub fn ouvrir_site(texte: &str) -> Result<(), String> {
    let adresse = adresse_tapee(texte).ok_or("Adresse refusée.")?;
    lancer(
        "firefox",
        &[OsStr::new("--new-tab"), OsStr::new(&adresse)],
        None,
    )
}

/// Ouvre une adresse web dans Firefox. Seules les adresses `http(s)://` passent : jamais un
/// argument qui commencerait par `-` ni un `javascript:`.
pub fn ouvrir_adresse(url: &str) -> Result<(), String> {
    if !adresse_web(url) {
        return Err("Adresse refusée.".into());
    }
    lancer("firefox", &[OsStr::new(url)], None)
}

fn adresse_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

#[cfg(test)]
mod tests {
    #[test]
    fn adresse_de_recherche_encodee() {
        assert_eq!(
            adresse_de_recherche(
                "https://www.ecosia.org/search?q={searchTerms}&addon=firefox",
                " cve php&x=é "
            )
            .as_deref(),
            Some("https://www.ecosia.org/search?q=cve%20php%26x%3D%C3%A9&addon=firefox")
        );
        assert_eq!(adresse_de_recherche("javascript:{searchTerms}", "a"), None);
        assert_eq!(adresse_de_recherche("https://exemple.fr/", "a"), None);
        assert_eq!(
            adresse_de_recherche("https://exemple.fr/?q={searchTerms}", "  "),
            None
        );
    }

    #[test]
    fn adresses_tapees() {
        assert_eq!(
            adresse_tapee("github.com").as_deref(),
            Some("https://github.com")
        );
        assert_eq!(
            adresse_tapee(" www.exemple.test/a?b=c ").as_deref(),
            Some("https://www.exemple.test/a?b=c")
        );
        assert_eq!(
            adresse_tapee("localhost.dev:3000/x").as_deref(),
            Some("https://localhost.dev:3000/x")
        );
        assert_eq!(
            adresse_tapee("http://192.168.1.2:8080").as_deref(),
            Some("http://192.168.1.2:8080")
        );
        assert_eq!(adresse_tapee("rapport.pdf"), None);
        assert_eq!(adresse_tapee("cve php"), None);
        assert_eq!(
            adresse_tapee("-profile.com"),
            Some("https://-profile.com".into())
        );
        assert_eq!(adresse_tapee("a..com"), None);
        assert_eq!(adresse_tapee("javascript:alert(1)"), None);
        assert_eq!(adresse_tapee("https://"), None);
    }

    #[test]
    fn recherche_web_en_argument_separe() {
        assert_eq!(
            arguments_recherche_web("  météo Paris ; rm -rf ~ "),
            Some(["--search".to_string(), "météo Paris ; rm -rf ~".to_string()])
        );
        // Jamais lu comme une option de Firefox.
        assert_eq!(
            arguments_recherche_web("--new-instance"),
            Some(["--search".to_string(), " --new-instance".to_string()])
        );
        assert_eq!(arguments_recherche_web("   "), None);
        assert_eq!(arguments_recherche_web("a\nb"), None);
        assert_eq!(arguments_recherche_web(&"x".repeat(501)), None);
    }

    use super::*;

    #[test]
    fn seules_les_adresses_web() {
        assert!(adresse_web("https://linkedin.com/"));
        assert!(!adresse_web("-profile /tmp"));
        assert!(!adresse_web("javascript:alert(1)"));
        assert!(!adresse_web("file:///etc/passwd"));
    }

    fn env(paires: &[(&str, &str)]) -> Vec<(String, String)> {
        paires
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn hors_appimage_rien_ne_change() {
        let variables = env(&[("GTK_THEME", "Mint-Y"), ("PATH", "/usr/bin")]);
        assert_eq!(environnement_propre(variables.clone(), None), variables);
    }

    #[test]
    fn retire_ce_que_l_appimage_a_pose() {
        let variables = env(&[
            ("APPDIR", "/tmp/.mount_Barre"),
            ("APPIMAGE", "/home/a/Applications/BarreRecherche.AppImage"),
            ("GTK_THEME", "Adwaita:light"),
            (
                "GDK_PIXBUF_MODULE_FILE",
                "/tmp/.mount_Barre//usr/lib/loaders.cache",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_Barre/usr/share:/usr/share:/usr/share/cinnamon:/usr/share",
            ),
            ("LD_LIBRARY_PATH", "/tmp/.mount_Barre/usr/lib"),
            ("PATH", "/tmp/.mount_Barre/usr/bin:/usr/local/bin:/usr/bin"),
            ("HOME", "/home/a"),
            ("AUTRE", "/tmp/.mount_Barrette/x"),
        ]);
        let propre = environnement_propre(variables, Some("/tmp/.mount_Barre"));
        assert_eq!(
            propre,
            env(&[
                ("XDG_DATA_DIRS", "/usr/share/cinnamon:/usr/share"),
                ("PATH", "/usr/local/bin:/usr/bin"),
                ("HOME", "/home/a"),
                ("AUTRE", "/tmp/.mount_Barrette/x"),
            ])
        );
    }

    #[test]
    fn garde_usr_share_s_il_n_est_qu_en_tete() {
        let variables = env(&[("XDG_DATA_DIRS", "/opt/a/usr/share:/usr/share:")]);
        let propre = environnement_propre(variables, Some("/opt/a"));
        assert_eq!(propre, env(&[("XDG_DATA_DIRS", "/usr/share:")]));
    }

    #[test]
    fn lance_sans_shell_avec_les_arguments_tels_quels() {
        let dossier = tempfile::tempdir().unwrap();
        let cible = dossier.path().join("a b; touch pirate");
        lancer("touch", &[cible.as_os_str()], Some(dossier.path())).unwrap();
        for _ in 0..100 {
            if cible.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(cible.exists());
        assert!(!dossier.path().join("pirate").exists());
    }
}
