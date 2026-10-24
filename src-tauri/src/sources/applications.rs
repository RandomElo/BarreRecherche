//! Applications : fichiers `.desktop` des dossiers `applications` (spécification §3), filtrés comme le menu
//! de Cinnamon. Le contenu de ces fichiers n'est pas de confiance (règle 4) : il n'est que lu et
//! affiché comme du texte ; le lancement passe par `gio launch` avec le chemin du fichier.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Une application du menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    /// Identifiant du fichier selon la spécification (chemin sous `applications/`, `/` remplacés par `-`).
    pub id: String,
    pub nom: String,
    /// Valeur de `Icon=` : un nom de l'icône du thème, ou un chemin absolu.
    pub icone: Option<String>,
    /// Nom générique et mots-clés traduits (`GenericName`, `Keywords`) : « navigateur » trouve Firefox.
    pub mots_cles: Vec<String>,
    /// `Comment=` traduit : montré dans l'aperçu (J11).
    pub description: Option<String>,
    pub fichier: PathBuf,
    /// Le programme de `Exec=` existe sur le poste (après `env` et ses `NOM=valeur`) : un lanceur
    /// recopié d'un autre poste peut pointer sur un logiciel absent.
    pub lancable: bool,
    /// Un des logiciels d'Eloi (`logiciels.json`) : section « Logiciels » plutôt qu'« Applications ».
    pub mien: bool,
}

/// Ce qu'il faut savoir du poste pour filtrer et traduire.
pub struct Contexte<'a> {
    /// Langues à essayer, de la plus précise à la plus large (`fr_FR`, `fr`).
    pub langues: &'a [String],
    /// Bureaux courants (`XDG_CURRENT_DESKTOP`, ex. `X-Cinnamon`), pour `OnlyShowIn` et `NotShowIn`.
    pub bureaux: &'a [String],
    /// Dossiers du `PATH`, pour `TryExec`.
    pub chemins: &'a [PathBuf],
}

/// Langues à essayer d'après `LC_ALL`, `LC_MESSAGES`, `LANG` (`fr_FR.UTF-8@euro` donne `fr_FR`, `fr`).
pub fn langues(valeur: Option<&str>) -> Vec<String> {
    let Some(valeur) = valeur.filter(|v| !v.is_empty() && *v != "C" && *v != "POSIX") else {
        return Vec::new();
    };
    let sans_modificateur = valeur.split('@').next().unwrap_or(valeur);
    let sans_encodage = sans_modificateur
        .split('.')
        .next()
        .unwrap_or(sans_modificateur);
    let mut resultat = vec![sans_encodage.to_string()];
    if let Some((langue, _pays)) = sans_encodage.split_once('_') {
        resultat.push(langue.to_string());
    }
    resultat
}

/// Défait les échappements d'une valeur de chaîne : `\s`, `\n`, `\t`, `\r`, `\\`.
fn deseschapper(valeur: &str) -> String {
    let mut resultat = String::with_capacity(valeur.len());
    let mut caracteres = valeur.chars();
    while let Some(c) = caracteres.next() {
        if c != '\\' {
            resultat.push(c);
            continue;
        }
        match caracteres.next() {
            Some('s') => resultat.push(' '),
            Some('n') => resultat.push('\n'),
            Some('t') => resultat.push('\t'),
            Some('r') => resultat.push('\r'),
            Some('\\') => resultat.push('\\'),
            Some(autre) => {
                resultat.push('\\');
                resultat.push(autre);
            }
            None => resultat.push('\\'),
        }
    }
    resultat
}

/// Clés du groupe `[Desktop Entry]` seulement (les actions `[Desktop Action …]` sont ignorées).
fn lire_groupe_principal(contenu: &str) -> HashMap<String, String> {
    let mut cles = HashMap::new();
    let mut dans_le_groupe = false;
    for ligne in contenu.lines() {
        let ligne = ligne.trim();
        if ligne.is_empty() || ligne.starts_with('#') {
            continue;
        }
        if ligne.starts_with('[') {
            dans_le_groupe = ligne == "[Desktop Entry]";
            continue;
        }
        if dans_le_groupe {
            if let Some((cle, valeur)) = ligne.split_once('=') {
                cles.entry(cle.trim().to_string())
                    .or_insert_with(|| valeur.trim().to_string());
            }
        }
    }
    cles
}

fn vrai(cles: &HashMap<String, String>, cle: &str) -> bool {
    cles.get(cle).is_some_and(|v| v == "true")
}

fn liste(valeur: &str) -> impl Iterator<Item = &str> {
    valeur.split(';').map(str::trim).filter(|v| !v.is_empty())
}

fn executable_trouve(programme: &str, chemins: &[PathBuf]) -> bool {
    let est_executable = |chemin: &Path| {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(chemin).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if programme.contains('/') {
        return est_executable(Path::new(programme));
    }
    chemins
        .iter()
        .any(|dossier| est_executable(&dossier.join(programme)))
}

/// Mots de `Exec=`, guillemets doubles retirés (spécification des lanceurs : `\"`, `\\` échappés).
fn mots_d_exec(exec: &str) -> Vec<String> {
    let mut mots = Vec::new();
    let mut mot = String::new();
    let mut entre_guillemets = false;
    let mut caracteres = exec.chars();
    while let Some(c) = caracteres.next() {
        match c {
            '"' => entre_guillemets = !entre_guillemets,
            '\\' if entre_guillemets => mot.extend(caracteres.next()),
            c if c.is_whitespace() && !entre_guillemets => {
                if !mot.is_empty() {
                    mots.push(std::mem::take(&mut mot));
                }
            }
            c => mot.push(c),
        }
    }
    if !mot.is_empty() {
        mots.push(mot);
    }
    mots
}

/// Programme lancé par `Exec=` : le premier mot, ou celui après `env` et ses `NOM=valeur`.
pub fn programme_d_exec(exec: &str) -> Option<String> {
    let mots = mots_d_exec(exec);
    let mut reste = mots.iter();
    let premier = reste.next()?;
    if Path::new(premier).file_name().is_some_and(|n| n == "env") {
        return reste
            .find(|mot| !mot.contains('=') && !mot.starts_with('-'))
            .cloned();
    }
    Some(premier.clone())
}

/// Application décrite par ce contenu, ou `None` si le menu ne la montrerait pas.
pub fn lire(id: &str, fichier: &Path, contenu: &str, contexte: &Contexte) -> Option<Application> {
    let cles = lire_groupe_principal(contenu);
    if cles.get("Type").map(String::as_str) != Some("Application")
        || vrai(&cles, "NoDisplay")
        || vrai(&cles, "Hidden")
    {
        return None;
    }
    if let Some(seulement) = cles.get("OnlyShowIn") {
        if !liste(seulement).any(|b| contexte.bureaux.iter().any(|c| c == b)) {
            return None;
        }
    }
    if let Some(sauf) = cles.get("NotShowIn") {
        if liste(sauf).any(|b| contexte.bureaux.iter().any(|c| c == b)) {
            return None;
        }
    }
    if let Some(essai) = cles.get("TryExec") {
        if !executable_trouve(&deseschapper(essai), contexte.chemins) {
            return None;
        }
    }
    let traduit = |cle: &str| {
        contexte
            .langues
            .iter()
            .find_map(|langue| cles.get(&format!("{cle}[{langue}]")))
            .or_else(|| cles.get(cle))
            .map(|v| deseschapper(v))
    };
    let nom = traduit("Name").filter(|v| !v.trim().is_empty())?;
    let mut mots_cles: Vec<String> = traduit("GenericName").into_iter().collect();
    if let Some(liste_de_mots) = traduit("Keywords") {
        mots_cles.extend(liste(&liste_de_mots).map(String::from));
    }
    mots_cles.retain(|mot| !mot.trim().is_empty() && *mot != nom);
    mots_cles.dedup();
    let icone = cles
        .get("Icon")
        .map(|v| deseschapper(v))
        .filter(|v| !v.is_empty());
    let description = traduit("Comment").filter(|v| !v.trim().is_empty());
    let lancable = cles
        .get("Exec")
        .and_then(|exec| programme_d_exec(exec))
        .is_some_and(|programme| executable_trouve(&programme, contexte.chemins));
    Some(Application {
        id: id.to_string(),
        nom,
        icone,
        mots_cles,
        description,
        fichier: fichier.to_path_buf(),
        lancable,
        mien: false,
    })
}

fn parcourir(dossier: &Path, base: &Path, trouves: &mut Vec<(String, PathBuf)>, profondeur: u8) {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    for entree in entrees.flatten() {
        let chemin = entree.path();
        let Ok(type_) = entree.file_type() else {
            continue;
        };
        if type_.is_dir() || (type_.is_symlink() && chemin.is_dir()) {
            if profondeur < 4 {
                parcourir(&chemin, base, trouves, profondeur + 1);
            }
        } else if chemin.extension().is_some_and(|e| e == "desktop") {
            if let Ok(relatif) = chemin.strip_prefix(base) {
                let id = relatif.to_string_lossy().replace('/', "-");
                trouves.push((id, chemin));
            }
        }
    }
}

/// Applications de ces dossiers de données (`XDG_DATA_HOME` en premier, puis `XDG_DATA_DIRS`) :
/// à identifiant égal, le premier dossier gagne, comme dans le menu. Triées par nom.
pub fn charger(dossiers_donnees: &[PathBuf], contexte: &Contexte) -> Vec<Application> {
    let mut vus = std::collections::HashSet::new();
    let mut applications = Vec::new();
    for donnees in dossiers_donnees {
        let base = donnees.join("applications");
        let mut trouves = Vec::new();
        parcourir(&base, &base, &mut trouves, 0);
        trouves.sort();
        for (id, fichier) in trouves {
            if !vus.insert(id.clone()) {
                continue;
            }
            // Fichier trop gros pour être une vraie entrée : ignoré.
            if fs::metadata(&fichier).map_or(true, |m| m.len() > 256 * 1024) {
                continue;
            }
            let Ok(contenu) = fs::read_to_string(&fichier) else {
                continue;
            };
            if let Some(application) = lire(&id, &fichier, &contenu, contexte) {
                applications.push(application);
            }
        }
    }
    applications.sort_by_key(|a| a.nom.to_lowercase());
    applications
}

#[cfg(test)]
mod tests {
    #[test]
    fn programme_du_lanceur() {
        assert_eq!(programme_d_exec("nemo %U").as_deref(), Some("nemo"));
        assert_eq!(
            programme_d_exec("\"/home/a/Applications/Notes.AppImage\" %U").as_deref(),
            Some("/home/a/Applications/Notes.AppImage")
        );
        // Un lanceur qui passe par `env` : le programme est après les variables.
        assert_eq!(
            programme_d_exec("env ANALYSE_NODE=\"/a/b/node\" \"/a/projet/scripts/lancer.sh\"")
                .as_deref(),
            Some("/a/projet/scripts/lancer.sh")
        );
        assert_eq!(programme_d_exec("   "), None);
    }

    use super::*;

    fn contexte_fr<'a>(langues: &'a [String], bureaux: &'a [String]) -> Contexte<'a> {
        Contexte {
            langues,
            bureaux,
            chemins: &[],
        }
    }

    fn lire_simple(contenu: &str) -> Option<Application> {
        let langues = langues(Some("fr_FR.UTF-8"));
        let bureaux = vec!["X-Cinnamon".to_string()];
        lire(
            "a.desktop",
            Path::new("/x/a.desktop"),
            contenu,
            &contexte_fr(&langues, &bureaux),
        )
    }

    #[test]
    fn langues_depuis_la_locale() {
        assert_eq!(langues(Some("fr_FR.UTF-8@euro")), ["fr_FR", "fr"]);
        assert_eq!(langues(Some("de")), ["de"]);
        assert!(langues(Some("C")).is_empty());
        assert!(langues(None).is_empty());
    }

    #[test]
    fn nom_traduit_sinon_nom_par_defaut() {
        let app = lire_simple(
            "[Desktop Entry]\nType=Application\nName=Files\nName[fr]=Fichiers\nIcon=folder\nExec=nemo\n",
        )
        .unwrap();
        assert_eq!(app.nom, "Fichiers");
        assert_eq!(app.icone.as_deref(), Some("folder"));
        let app = lire_simple("[Desktop Entry]\nType=Application\nName=Calc\n").unwrap();
        assert_eq!(app.nom, "Calc");
        assert_eq!(app.icone, None);
        let app =
            lire_simple("[Desktop Entry]\nType=Application\nName=A\nName[fr]=B\nName[fr_FR]=C\n")
                .unwrap();
        assert_eq!(app.nom, "C");
    }

    #[test]
    fn nom_generique_et_mots_cles_traduits() {
        let app = lire_simple(
            "[Desktop Entry]\nType=Application\nName=Firefox\nGenericName=Web Browser\nGenericName[fr]=Navigateur Web\nKeywords=Internet;WWW\nKeywords[fr]=Internet;Fureteur;\n",
        )
        .unwrap();
        assert_eq!(app.mots_cles, ["Navigateur Web", "Internet", "Fureteur"]);
        let app = lire_simple("[Desktop Entry]\nType=Application\nName=A\n").unwrap();
        assert!(app.mots_cles.is_empty());
    }

    #[test]
    fn ignore_les_actions_et_les_commentaires() {
        let app = lire_simple(
            "# Name=Faux\n[Desktop Entry]\nType=Application\nName=Firefox\n\n[Desktop Action new-window]\nName=Nouvelle fenêtre\n",
        )
        .unwrap();
        assert_eq!(app.nom, "Firefox");
    }

    #[test]
    fn cache_ce_que_le_menu_cache() {
        assert!(lire_simple("[Desktop Entry]\nType=Link\nName=Lien\n").is_none());
        assert!(
            lire_simple("[Desktop Entry]\nType=Application\nName=A\nNoDisplay=true\n").is_none()
        );
        assert!(lire_simple("[Desktop Entry]\nType=Application\nName=A\nHidden=true\n").is_none());
        assert!(
            lire_simple("[Desktop Entry]\nType=Application\nName=A\nOnlyShowIn=GNOME;KDE;\n")
                .is_none()
        );
        assert!(
            lire_simple("[Desktop Entry]\nType=Application\nName=A\nOnlyShowIn=X-Cinnamon;\n")
                .is_some()
        );
        assert!(
            lire_simple("[Desktop Entry]\nType=Application\nName=A\nNotShowIn=X-Cinnamon;\n")
                .is_none()
        );
        assert!(lire_simple(
            "[Desktop Entry]\nType=Application\nName=A\nTryExec=programme-absent-xyz\n"
        )
        .is_none());
        assert!(lire_simple("[Desktop Entry]\nType=Application\nName=\n").is_none());
    }

    #[test]
    fn valeurs_deseschappees() {
        let app = lire_simple("[Desktop Entry]\nType=Application\nName=A\\sB\\\\C\n").unwrap();
        assert_eq!(app.nom, "A B\\C");
    }

    #[test]
    fn premier_dossier_gagne_et_sous_dossiers_en_identifiant() {
        let racine = tempfile::tempdir().unwrap();
        let utilisateur = racine.path().join("home");
        let systeme = racine.path().join("usr");
        for (dossier, nom) in [(&utilisateur, "Perso"), (&systeme, "Système")] {
            fs::create_dir_all(dossier.join("applications/kde")).unwrap();
            fs::write(
                dossier.join("applications/editeur.desktop"),
                format!("[Desktop Entry]\nType=Application\nName={nom}\n"),
            )
            .unwrap();
        }
        fs::write(
            systeme.join("applications/kde/outil.desktop"),
            "[Desktop Entry]\nType=Application\nName=Outil\n",
        )
        .unwrap();
        let langues = langues(Some("fr_FR.UTF-8"));
        let bureaux: Vec<String> = vec![];
        let apps = charger(&[utilisateur, systeme], &contexte_fr(&langues, &bureaux));
        let resume: Vec<_> = apps
            .iter()
            .map(|a| (a.id.as_str(), a.nom.as_str()))
            .collect();
        assert_eq!(
            resume,
            [("kde-outil.desktop", "Outil"), ("editeur.desktop", "Perso")]
        );
    }
}
