//! Fenêtre des réglages, ouverte par l'engrenage de la barre : créée à la demande, détruite à la fermeture.

use crate::{barre, demarrage, raccourci};
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
    let demarrage_auto = demarrage::appimage()
        .and(demarrage::dossier_autostart())
        .map(|dossier| demarrage::est_actif(&dossier));
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
    let resultat = if actif {
        demarrage::activer(&dossier, &appimage)
    } else {
        demarrage::desactiver(&dossier)
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
