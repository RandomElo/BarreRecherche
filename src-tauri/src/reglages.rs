//! Fenêtre des réglages, ouverte par l'engrenage de la barre : créée à la demande, détruite à la fermeture.

use crate::{barre, demarrage, raccourci, service};
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const ETIQUETTE: &str = "reglages";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatReglages {
    raccourci: &'static str,
    raccourci_actif: bool,
    /// `None` hors AppImage : il n'y a rien à lancer avec la session.
    demarrage_auto: Option<bool>,
}

fn etat(app: &AppHandle) -> EtatReglages {
    let demarrage_auto = demarrage::appimage().map(|_| {
        service::dossier_unites().is_some_and(|dossier| service::est_actif(&dossier))
            || demarrage::dossier_autostart().is_some_and(|dossier| demarrage::est_actif(&dossier))
    });
    EtatReglages {
        raccourci: raccourci::LIBELLE,
        raccourci_actif: app.state::<raccourci::EtatRaccourci>().actif(),
        demarrage_auto,
    }
}

#[tauri::command]
pub fn etat_reglages(app: AppHandle) -> EtatReglages {
    etat(&app)
}

#[tauri::command]
pub fn definir_demarrage_auto(app: AppHandle, actif: bool) -> Result<EtatReglages, String> {
    let (Some(appimage), Some(dossier)) = (demarrage::appimage(), demarrage::dossier_autostart())
    else {
        return Err("Le démarrage avec la session se règle depuis l'AppImage.".into());
    };
    // Par le service de la session (relancé s'il plante) ; sans systemd, l'ancien fichier `.desktop`.
    let resultat = match (actif, service::dossier_unites()) {
        (true, Some(unites)) => service::activer(&unites, &appimage, Some(&dossier))
            .or_else(|_| demarrage::activer(&dossier, &appimage)),
        (true, None) => demarrage::activer(&dossier, &appimage),
        (false, unites) => unites
            .map_or(Ok(()), |unites| service::desactiver(&unites))
            .and_then(|()| demarrage::desactiver(&dossier)),
    };
    resultat.map_err(|erreur| format!("Réglage non enregistré : {erreur}"))?;
    Ok(etat(&app))
}

/// Async : créer une fenêtre depuis une commande synchrone peut bloquer la boucle d'événements.
#[tauri::command]
pub async fn ouvrir_reglages(app: AppHandle) -> Result<(), String> {
    if let Some(fenetre) = app.get_webview_window(barre::ETIQUETTE) {
        let _ = fenetre.hide();
    }
    if let Some(fenetre) = app.get_webview_window(ETIQUETTE) {
        let _ = fenetre.show();
        let _ = fenetre.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, ETIQUETTE, WebviewUrl::App("reglages.html".into()))
        .title("Réglages de la barre de recherche")
        .inner_size(480.0, 340.0)
        .resizable(false)
        .center()
        .build()
        .map(|_| ())
        .map_err(|erreur| erreur.to_string())
}

#[tauri::command]
pub fn quitter(app: AppHandle) {
    app.exit(0);
}

/// Relance la barre : par le service s'il l'a lancée, sinon par un nouveau processus lancé une seconde
/// plus tard (le temps que celui-ci s'arrête). Le menu Windows reste vidé pendant ce temps.
#[tauri::command]
pub fn redemarrer() -> Result<(), String> {
    if service::lance_par_le_service() {
        return if service::redemarrer() {
            Ok(())
        } else {
            Err("Le service n'a pas pu être relancé.".into())
        };
    }
    let Some(appimage) = demarrage::appimage() else {
        return Err("Le redémarrage se fait depuis l'AppImage.".into());
    };
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    // Arguments fixes : le chemin de l'AppImage est passé en paramètre, jamais collé dans le script.
    Command::new("sh")
        .args(["-c", "sleep 1; exec \"$0\" --fond", &appimage])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|erreur| format!("Redémarrage impossible : {erreur}"))?;
    std::process::exit(0);
}
