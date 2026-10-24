//! Démarrage avec la session : un fichier `.desktop` dans `~/.config/autostart/`, écrit seulement
//! quand la case des réglages est cochée, et seulement depuis l'AppImage (en développement, rien à régler).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const NOM_FICHIER: &str = "barre-recherche.desktop";

/// Chemin de l'AppImage en cours d'exécution, donné par son lanceur. Absent en développement.
pub fn appimage() -> Option<String> {
    std::env::var("APPIMAGE")
        .ok()
        .filter(|chemin| !chemin.is_empty())
}

/// Dossier `autostart` de la session : `$XDG_CONFIG_HOME/autostart`, sinon `~/.config/autostart`.
pub fn dossier_autostart() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|valeur| !valeur.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("autostart"))
}

/// Argument entre guillemets pour la clé `Exec`, échappé selon la spécification des fichiers `.desktop` :
/// `"`, `` ` ``, `$` et `\` précédés d'une barre oblique inverse dans les guillemets, puis chaque `\`
/// doublé parce que la valeur est une chaîne, et `%` doublé pour ne pas être lu comme un code de champ.
pub fn argument_exec(chemin: &str) -> String {
    let mut entre_guillemets = String::new();
    for caractere in chemin.chars() {
        if matches!(caractere, '"' | '`' | '$' | '\\') {
            entre_guillemets.push('\\');
        }
        entre_guillemets.push(caractere);
    }
    let chaine = entre_guillemets.replace('\\', "\\\\").replace('%', "%%");
    format!("\"{chaine}\"")
}

/// Contenu du fichier : lance l'AppImage en fond, barre cachée.
pub fn contenu(appimage: &str) -> String {
    let chemin_sur_une_ligne = appimage.replace(['\n', '\r'], "");
    format!(
        "[Desktop Entry]\nType=Application\nName=Barre de recherche\nComment=Barre de recherche du poste, ouverte par Ctrl+K\nExec={} --fond\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n",
        argument_exec(&chemin_sur_une_ligne)
    )
}

pub fn est_actif(dossier: &Path) -> bool {
    dossier.join(NOM_FICHIER).is_file()
}

pub fn activer(dossier: &Path, appimage: &str) -> io::Result<()> {
    fs::create_dir_all(dossier)?;
    fs::write(dossier.join(NOM_FICHIER), contenu(appimage))
}

pub fn desactiver(dossier: &Path) -> io::Result<()> {
    match fs::remove_file(dossier.join(NOM_FICHIER)) {
        Err(erreur) if erreur.kind() == io::ErrorKind::NotFound => Ok(()),
        autre => autre,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chemin_simple_entre_guillemets() {
        assert_eq!(
            argument_exec("/home/a/Applications/BarreRecherche.AppImage"),
            "\"/home/a/Applications/BarreRecherche.AppImage\""
        );
    }

    #[test]
    fn caracteres_speciaux_echappes() {
        assert_eq!(argument_exec("/a b/\"x\""), "\"/a b/\\\\\"x\\\\\"\"");
        assert_eq!(argument_exec("/$HOME/`id`"), "\"/\\\\$HOME/\\\\`id\\\\`\"");
        assert_eq!(argument_exec("/a\\b"), "\"/a\\\\\\\\b\"");
        assert_eq!(argument_exec("/100%/x"), "\"/100%%/x\"");
    }

    #[test]
    fn contenu_lance_en_fond_sans_saut_de_ligne_injecte() {
        let texte = contenu("/x/B.AppImage\nExec=autre");
        assert!(texte.contains("Exec=\"/x/B.AppImageExec=autre\" --fond\n"));
        assert_eq!(texte.matches("Exec=").count(), 2);
        assert_eq!(texte.lines().filter(|l| l.starts_with("Exec=")).count(), 1);
    }

    #[test]
    fn activer_puis_desactiver() {
        let dossier = tempfile::tempdir().unwrap();
        let autostart = dossier.path().join("autostart");
        assert!(!est_actif(&autostart));
        activer(&autostart, "/x/B.AppImage").unwrap();
        assert!(est_actif(&autostart));
        desactiver(&autostart).unwrap();
        assert!(!est_actif(&autostart));
        desactiver(&autostart).unwrap();
    }
}
