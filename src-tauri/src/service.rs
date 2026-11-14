//! La barre tourne comme un service de la session (systemd, côté utilisateur) quand « Démarrer avec la
//! session » est coché : elle est relancée si elle plante, et si elle plante en boucle, le service
//! s'arrête et un second service rend le menu Windows de Cinnamon et prévient (`--rendre-le-menu`).
//!
//! Fichiers écrits dans `~/.config/systemd/user/` : `barre-recherche.service` et
//! `barre-recherche-secours.service`. Remplace le fichier `.desktop` de `~/.config/autostart/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SERVICE: &str = "barre-recherche.service";
const SECOURS: &str = "barre-recherche-secours.service";

/// Dossier des unités de la session : `$XDG_CONFIG_HOME/systemd/user`, sinon `~/.config/systemd/user`.
pub fn dossier_unites() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|valeur| !valeur.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("systemd/user"))
}

/// Le chemin va tel quel dans `ExecStart` : refusé s'il contient de quoi être lu autrement par
/// systemd (guillemet, barre oblique inverse, `$`, `%`, saut de ligne).
fn chemin_sur(appimage: &str) -> Result<&str, String> {
    if appimage.is_empty()
        || appimage
            .chars()
            .any(|c| matches!(c, '"' | '\\' | '$' | '%' | '\n' | '\r') || c.is_control())
    {
        return Err(
            "Le chemin de l'AppImage contient un caractère que le service ne sait pas écrire."
                .into(),
        );
    }
    Ok(appimage)
}

pub fn contenu_service(appimage: &str) -> String {
    format!(
        "[Unit]\n\
         Description=Barre de recherche\n\
         After=graphical-session.target\n\
         StartLimitIntervalSec=120\n\
         StartLimitBurst=4\n\
         OnFailure={SECOURS}\n\
         \n\
         [Service]\n\
         ExecStart=\"{appimage}\" --fond\n\
         Restart=on-failure\n\
         RestartSec=2\n\
         # Les logiciels lancés depuis la barre survivent à un arrêt ou à un redémarrage de la barre.\n\
         KillMode=process\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

pub fn contenu_secours(appimage: &str) -> String {
    format!(
        "[Unit]\n\
         Description=Barre de recherche : rend le menu Windows après des plantages en boucle\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         ExecStart=\"{appimage}\" --rendre-le-menu --plantage\n"
    )
}

fn systemctl(arguments: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|statut| statut.success())
}

pub fn est_actif(dossier: &Path) -> bool {
    dossier.join(SERVICE).is_file()
}

/// Écrit les deux unités (seulement si elles ont changé). Vrai si quelque chose a été écrit.
fn ecrire(dossier: &Path, appimage: &str) -> io::Result<bool> {
    let appimage = chemin_sur(appimage).map_err(io::Error::other)?;
    fs::create_dir_all(dossier)?;
    let mut change = false;
    for (nom, contenu) in [
        (SERVICE, contenu_service(appimage)),
        (SECOURS, contenu_secours(appimage)),
    ] {
        let chemin = dossier.join(nom);
        if fs::read_to_string(&chemin).ok().as_deref() != Some(contenu.as_str()) {
            fs::write(chemin, contenu)?;
            change = true;
        }
    }
    Ok(change)
}

/// Coche « Démarrer avec la session » : unités écrites et activées, ancien `.desktop` retiré.
pub fn activer(dossier: &Path, appimage: &str, autostart: Option<&Path>) -> io::Result<()> {
    ecrire(dossier, appimage)?;
    if let Some(autostart) = autostart {
        crate::demarrage::desactiver(autostart)?;
    }
    systemctl(&["daemon-reload"]);
    if !systemctl(&["enable", SERVICE]) {
        return Err(io::Error::other("systemctl --user enable a échoué"));
    }
    Ok(())
}

/// Décoche : service désactivé, unités retirées.
pub fn desactiver(dossier: &Path) -> io::Result<()> {
    systemctl(&["disable", SERVICE]);
    for nom in [SERVICE, SECOURS] {
        match fs::remove_file(dossier.join(nom)) {
            Err(erreur) if erreur.kind() != io::ErrorKind::NotFound => return Err(erreur),
            _ => {}
        }
    }
    systemctl(&["daemon-reload"]);
    Ok(())
}

/// Au démarrage de la barre : passe de l'ancien `.desktop` au service, et tient les unités à jour
/// (l'AppImage a pu changer de place).
pub fn mettre_a_jour(dossier: &Path, appimage: &str, autostart: Option<&Path>) {
    let ancien = autostart.is_some_and(crate::demarrage::est_actif);
    if ancien {
        let _ = activer(dossier, appimage, autostart);
    } else if est_actif(dossier) && ecrire(dossier, appimage).unwrap_or(false) {
        systemctl(&["daemon-reload"]);
    }
}

/// Vrai si le service est tombé en échec et que systemd ne le relance plus.
pub fn en_echec() -> bool {
    systemctl(&["is-failed", SERVICE])
}

/// Vrai si ce processus est celui que le service a lancé.
pub fn lance_par_le_service() -> bool {
    let sortie = Command::new("systemctl")
        .args(["--user", "show", "-p", "MainPID", "--value", SERVICE])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    sortie
        .is_ok_and(|s| String::from_utf8_lossy(&s.stdout).trim() == std::process::id().to_string())
}

/// Redémarre la barre par le service (elle est arrêtée puis relancée par systemd).
pub fn redemarrer() -> bool {
    systemctl(&["restart", "--no-block", SERVICE])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_relance_et_appelle_le_secours() {
        let texte = contenu_service("/x/B.AppImage");
        assert!(texte.contains("ExecStart=\"/x/B.AppImage\" --fond\n"));
        assert!(texte.contains("Restart=on-failure"));
        assert!(texte.contains("StartLimitBurst=4"));
        assert!(texte.contains(&format!("OnFailure={SECOURS}")));
        assert!(texte.contains("KillMode=process"));
        assert!(contenu_secours("/x/B.AppImage").contains("--rendre-le-menu"));
    }

    #[test]
    fn chemin_dangereux_refuse() {
        assert!(chemin_sur("/a b/c.AppImage").is_ok());
        for mauvais in ["/a\"b", "/a\\b", "/a$b", "/100%", "/a\nExecStart=x", ""] {
            assert!(chemin_sur(mauvais).is_err(), "{mauvais:?}");
        }
    }

    #[test]
    fn ecriture_seulement_si_changement() {
        let temporaire = tempfile::tempdir().unwrap();
        let dossier = temporaire.path().join("user");
        assert!(!est_actif(&dossier));
        assert!(ecrire(&dossier, "/x/B.AppImage").unwrap());
        assert!(est_actif(&dossier));
        assert!(!ecrire(&dossier, "/x/B.AppImage").unwrap());
        assert!(ecrire(&dossier, "/y/B.AppImage").unwrap());
        assert!(ecrire(&dossier, "/a\"b").is_err());
    }
}
