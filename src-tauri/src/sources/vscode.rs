//! Dossiers ouverts récemment dans VS Code (spécification §6 : `@vsc` au champ vide), lus sans y
//! écrire (règle 2) : chaque dossier ouvert a son dossier dans `workspaceStorage`, avec un
//! `workspace.json` qui le nomme (`"folder": "file:///…"`) ; la date de son `state.vscdb` dit sa
//! dernière utilisation. Les dossiers distants (`vscode-remote://`) sont laissés de côté.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Dossiers de réglages des variantes de VS Code, sous `~/.config`.
const VARIANTES: [&str; 3] = ["Code", "Code - OSS", "VSCodium"];

/// Dossiers ouverts dans VS Code, du plus récent au plus ancien, d'après `config` (`~/.config`).
pub fn dossiers_recents(config: &Path) -> Vec<PathBuf> {
    let mut trouves: Vec<(SystemTime, PathBuf)> = VARIANTES
        .iter()
        .filter_map(|variante| {
            std::fs::read_dir(config.join(variante).join("User/workspaceStorage")).ok()
        })
        .flatten()
        .flatten()
        .filter_map(|entree| {
            let espace = entree.path();
            let description = std::fs::read_to_string(espace.join("workspace.json")).ok()?;
            let dossier = dossier_du_workspace(&description)?;
            let date = std::fs::metadata(espace.join("state.vscdb"))
                .or_else(|_| entree.metadata())
                .and_then(|m| m.modified())
                .ok()?;
            Some((date, dossier))
        })
        .collect();
    trouves.sort_by(|(a, _), (b, _)| b.cmp(a));
    let mut dossiers: Vec<PathBuf> = Vec::new();
    for (_, dossier) in trouves {
        if !dossiers.contains(&dossier) {
            dossiers.push(dossier);
        }
    }
    dossiers
}

/// Dossier local nommé par un `workspace.json`, décodé ; `None` pour un dossier distant ou un
/// fichier `.code-workspace`.
pub fn dossier_du_workspace(description: &str) -> Option<PathBuf> {
    let valeur: serde_json::Value = serde_json::from_str(description).ok()?;
    let adresse = valeur.get("folder")?.as_str()?;
    crate::apercu::chemin_de_l_adresse(adresse.strip_prefix("file://")?)
}

/// `$XDG_CONFIG_HOME`, sinon `~/.config`.
pub fn dossier_config() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn dossier_local_decode_et_distant_ignore() {
        assert_eq!(
            dossier_du_workspace(r#"{"folder": "file:///home/a/_%C3%89cole/z%20ESGI"}"#),
            Some(PathBuf::from("/home/a/_École/z ESGI"))
        );
        assert_eq!(
            dossier_du_workspace(r#"{"folder": "vscode-remote://ssh-remote%2Bhote/home/a"}"#),
            None
        );
        assert_eq!(
            dossier_du_workspace(r#"{"workspace": "file:///home/a/x.code-workspace"}"#),
            None
        );
        assert_eq!(dossier_du_workspace("pas du json"), None);
    }

    #[test]
    fn du_plus_recent_au_plus_ancien() {
        let config = tempfile::tempdir().unwrap();
        let stockage = config.path().join("Code/User/workspaceStorage");
        let maintenant = SystemTime::now();
        for (nom, dossier, il_y_a) in [
            ("a1", "/p/ancien", 300),
            ("b2", "/p/recent", 10),
            ("c3", "/p/milieu", 100),
        ] {
            let espace = stockage.join(nom);
            std::fs::create_dir_all(&espace).unwrap();
            std::fs::write(
                espace.join("workspace.json"),
                format!(r#"{{"folder": "file://{dossier}"}}"#),
            )
            .unwrap();
            let base = std::fs::File::create(espace.join("state.vscdb")).unwrap();
            base.set_modified(maintenant - Duration::from_secs(il_y_a))
                .unwrap();
        }
        assert_eq!(
            dossiers_recents(config.path()),
            [
                PathBuf::from("/p/recent"),
                PathBuf::from("/p/milieu"),
                PathBuf::from("/p/ancien")
            ]
        );
        assert!(dossiers_recents(&config.path().join("absent")).is_empty());
    }
}
