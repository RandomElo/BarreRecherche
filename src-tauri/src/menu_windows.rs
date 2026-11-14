//! Le menu de Cinnamon s'ouvre lui aussi sur la touche Windows : tant que la barre tourne, son raccourci
//! est vidé (sinon le menu s'ouvre avec la barre et lui prend le clavier) ; quand la barre s'arrête,
//! il est rendu, pour que la touche Windows serve de nouveau.
//!
//! Le raccourci est un réglage de l'applet du menu : `overlay-key` dans
//! `~/.config/cinnamon/spices/menu@cinnamon.org/<n>.json`. Cinnamon ne relit pas le fichier tout seul :
//! l'applet est rechargée par D-Bus (`ReloadXlet`).

use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const APPLET: &str = "menu@cinnamon.org";
const REGLAGE: &str = "overlay-key";
/// Raccourci de l'applet quand rien d'autre n'est connu.
const VALEUR_PAR_DEFAUT: &str = "Super_L::Super_R";

fn dossier_personnel() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|valeur| !valeur.is_empty())
        .map(PathBuf::from)
}

fn dossier_reglages_applet() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|valeur| !valeur.is_empty())
        .map(PathBuf::from)
        .or_else(|| dossier_personnel().map(|home| home.join(".config")))?;
    Some(config.join("cinnamon/spices").join(APPLET))
}

/// Fichier où sont gardées les valeurs d'origine, pour les rendre telles quelles.
fn fichier_memoire() -> Option<PathBuf> {
    Some(dossier_personnel()?.join(".config/BarreRecherche/menu-windows.json"))
}

/// Position (début, fin) du texte entre guillemets de la clé `cle` dans le bloc `bloc`.
/// Lecture en texte : le fichier est réécrit tel quel, seule la valeur change.
fn valeur_de(texte: &str, bloc: &str, cle: &str) -> Option<(usize, usize)> {
    let debut_bloc = texte.find(&format!("\"{bloc}\""))?;
    let apres = &texte[debut_bloc..];
    let debut_cle = apres.find(&format!("\"{cle}\""))? + debut_bloc;
    let apres_cle = &texte[debut_cle + cle.len() + 2..];
    let deux_points = apres_cle.find(':')?;
    let reste = &apres_cle[deux_points + 1..];
    let ouvrant = reste.find('"')?;
    // Rien d'autre que des espaces entre les deux-points et le guillemet : c'est bien une chaîne.
    if !reste[..ouvrant].trim().is_empty() {
        return None;
    }
    let debut = debut_cle + cle.len() + 2 + deux_points + 1 + ouvrant + 1;
    let fermant = texte[debut..].find('"')?;
    Some((debut, debut + fermant))
}

fn lire_valeur(texte: &str, cle: &str) -> Option<String> {
    let (debut, fin) = valeur_de(texte, REGLAGE, cle)?;
    Some(texte[debut..fin].to_string())
}

fn remplacer_valeur(texte: &str, nouvelle: &str) -> Option<String> {
    let (debut, fin) = valeur_de(texte, REGLAGE, "value")?;
    Some(format!("{}{}{}", &texte[..debut], nouvelle, &texte[fin..]))
}

fn memoire_lue(fichier: &Path) -> Map<String, Value> {
    fs::read_to_string(fichier)
        .ok()
        .and_then(|texte| serde_json::from_str::<Value>(&texte).ok())
        .and_then(|valeur| valeur.as_object().cloned())
        .unwrap_or_default()
}

fn fichiers_json(dossier: &Path) -> Vec<PathBuf> {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut fichiers: Vec<PathBuf> = entrees
        .flatten()
        .map(|entree| entree.path())
        .filter(|chemin| chemin.extension().is_some_and(|e| e == "json"))
        .collect();
    fichiers.sort();
    fichiers
}

/// Recharge l'applet du menu pour qu'elle relise son raccourci.
fn recharger_l_applet() {
    // Les essais ne touchent pas à l'applet du vrai bureau.
    if cfg!(test) {
        return;
    }
    let _ = Command::new("dbus-send")
        .args([
            "--session",
            "--dest=org.Cinnamon",
            "--type=method_call",
            "/org/Cinnamon",
            "org.Cinnamon.ReloadXlet",
            &format!("string:{APPLET}"),
            "string:APPLET",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Vide le raccourci du menu (la touche Windows revient à la barre). Sans effet, et sans rechargement
/// de l'applet, quand il est déjà vide.
pub fn liberer() {
    let (Some(dossier), Some(memoire)) = (dossier_reglages_applet(), fichier_memoire()) else {
        return;
    };
    changer(&dossier, &memoire, false);
}

/// Rend le raccourci d'origine au menu (la touche Windows rouvre le menu de Cinnamon).
pub fn rendre() {
    let (Some(dossier), Some(memoire)) = (dossier_reglages_applet(), fichier_memoire()) else {
        return;
    };
    changer(&dossier, &memoire, true);
}

/// `rendre` vrai : remet le raccourci gardé ; faux : le garde puis le vide. Renvoie vrai si un fichier a changé.
fn changer(dossier: &Path, memoire: &Path, rendre: bool) -> bool {
    let mut gardees = memoire_lue(memoire);
    let mut change = false;
    for fichier in fichiers_json(dossier) {
        let Ok(texte) = fs::read_to_string(&fichier) else {
            continue;
        };
        let Some(actuelle) = lire_valeur(&texte, "value") else {
            continue;
        };
        let nom = fichier
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let voulue = if rendre {
            if !actuelle.is_empty() {
                continue;
            }
            gardees
                .get(&nom)
                .and_then(Value::as_str)
                .map(str::to_string)
                .filter(|v| !v.is_empty())
                .or_else(|| lire_valeur(&texte, "default").filter(|v| !v.is_empty()))
                .unwrap_or_else(|| VALEUR_PAR_DEFAUT.to_string())
        } else {
            if actuelle.is_empty() {
                continue;
            }
            gardees.insert(nom, Value::String(actuelle));
            String::new()
        };
        if let Some(nouveau) = remplacer_valeur(&texte, &voulue) {
            if fs::write(&fichier, nouveau).is_ok() {
                change = true;
            }
        }
    }
    if !rendre && change {
        if let Some(parent) = memoire.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(
            memoire,
            serde_json::to_string_pretty(&Value::Object(gardees)).unwrap_or_default(),
        );
    }
    if change {
        recharger_l_applet();
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGLAGES: &str = r#"{
    "overlay-key": {
        "type": "keybinding",
        "description": "Raccourci",
        "default": "Super_L::Super_R",
        "value": "Super_L::Super_R"
    },
    "autre": {
        "default": "x",
        "value": "y"
    }
}
"#;

    #[test]
    fn lit_et_remplace_seulement_la_valeur_du_raccourci() {
        assert_eq!(
            lire_valeur(REGLAGES, "value").as_deref(),
            Some("Super_L::Super_R")
        );
        let vide = remplacer_valeur(REGLAGES, "").unwrap();
        assert_eq!(lire_valeur(&vide, "value").as_deref(), Some(""));
        assert_eq!(
            lire_valeur(&vide, "default").as_deref(),
            Some("Super_L::Super_R")
        );
        assert!(vide.contains("\"value\": \"y\""));
        assert_eq!(vide.len(), REGLAGES.len() - "Super_L::Super_R".len());
    }

    #[test]
    fn liberer_puis_rendre_remet_le_raccourci_d_origine() {
        let temporaire = tempfile::tempdir().unwrap();
        let dossier = temporaire.path().join("menu");
        fs::create_dir_all(&dossier).unwrap();
        fs::write(dossier.join("0.json"), REGLAGES).unwrap();
        fs::write(
            dossier.join("1.json"),
            REGLAGES.replace("\"value\": \"Super_L::Super_R\"", "\"value\": \"<Super>m\""),
        )
        .unwrap();
        let memoire = temporaire.path().join("memoire.json");

        assert!(changer(&dossier, &memoire, false));
        for nom in ["0.json", "1.json"] {
            let texte = fs::read_to_string(dossier.join(nom)).unwrap();
            assert_eq!(lire_valeur(&texte, "value").as_deref(), Some(""));
        }
        // Déjà vide : rien à refaire.
        assert!(!changer(&dossier, &memoire, false));

        assert!(changer(&dossier, &memoire, true));
        let zero = fs::read_to_string(dossier.join("0.json")).unwrap();
        let un = fs::read_to_string(dossier.join("1.json")).unwrap();
        assert_eq!(
            lire_valeur(&zero, "value").as_deref(),
            Some("Super_L::Super_R")
        );
        assert_eq!(lire_valeur(&un, "value").as_deref(), Some("<Super>m"));
        // Rien d'autre n'a changé : rendre deux fois est sans effet.
        assert!(!changer(&dossier, &memoire, true));
    }

    #[test]
    fn rendre_sans_memoire_prend_le_defaut_de_l_applet() {
        let temporaire = tempfile::tempdir().unwrap();
        let dossier = temporaire.path().join("menu");
        fs::create_dir_all(&dossier).unwrap();
        let vide = remplacer_valeur(REGLAGES, "").unwrap();
        fs::write(dossier.join("0.json"), vide).unwrap();
        assert!(changer(
            &dossier,
            &temporaire.path().join("rien.json"),
            true
        ));
        let texte = fs::read_to_string(dossier.join("0.json")).unwrap();
        assert_eq!(
            lire_valeur(&texte, "value").as_deref(),
            Some("Super_L::Super_R")
        );
    }

    #[test]
    fn fichier_sans_raccourci_laisse_tel_quel() {
        assert!(lire_valeur("{\"autre\": {\"value\": \"y\"}}", "value").is_none());
    }
}
