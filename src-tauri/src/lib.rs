//! Partie système de la barre : programme résident, fenêtre cachée montrée par le raccourci global,
//! réglages, index des sources et lancement des logiciels.

mod apercu;
mod barre;
mod bourse;
mod cibles;
mod contexte;
mod demarrage;
mod floue;
mod icones;
mod lancement;
mod menu_windows;
mod raccourci;
mod recents;
mod recherche;
mod reglages;
mod service;
mod sources;
mod surveillance;
mod touche_windows;

use tauri::{Manager, WindowEvent};

/// Démarre le programme. Avec `--fond` (démarrage avec la session), la barre reste cachée ; avec
/// `--cible=<nom>` (la loupe de Notes : `--cible=notes`), elle s'ouvre avec cette cible posée.
pub fn lancer() {
    let arguments: Vec<String> = std::env::args().collect();
    // Lancé par le service de secours après des plantages en boucle : le menu Windows est rendu, rien d'autre.
    if arguments
        .iter()
        .any(|argument| argument == "--rendre-le-menu")
    {
        if arguments.iter().any(|argument| argument == "--plantage") {
            // Appelé à chaque plantage : tant que systemd relance la barre, il n'y a rien à faire ; elle
            // n'est « en échec » qu'une fois la limite de relances atteinte.
            std::thread::sleep(std::time::Duration::from_secs(6));
            if !service::en_echec() {
                return;
            }
            menu_windows::rendre();
            prevenir_du_plantage();
        } else {
            menu_windows::rendre();
        }
        return;
    }
    let en_fond = arguments.iter().any(|argument| argument == "--fond");
    let cible = contexte::cible_demandee(&arguments);

    tauri::Builder::default()
        // En premier : un second lancement montre la barre de l'instance déjà là, avec la cible
        // de `--cible=`, puis s'arrête.
        .plugin(tauri_plugin_single_instance::init(
            |app, arguments, _dossier| {
                contexte::ouvrir_par_lancement(app, &arguments);
            },
        ))
        // Pages HTML de l'aperçu : seulement des fichiers indexés, seulement pour la barre.
        .register_uri_scheme_protocol("apercu", |contexte, requete| {
            if contexte.webview_label() != barre::ETIQUETTE {
                return tauri::http::Response::builder()
                    .status(403)
                    .body(Vec::new())
                    .expect("réponse vide");
            }
            let index = contexte.app_handle().state::<recherche::Index>();
            apercu::servir(&index, requete.uri().path())
        })
        .manage(raccourci::EtatRaccourci::default())
        .manage(contexte::Contexte::default())
        .manage(recherche::Index::nouveau())
        .manage(bourse::Bourse::default())
        .invoke_handler(tauri::generate_handler![
            barre::ajuster_taille,
            barre::cacher_barre,
            reglages::ouvrir_reglages,
            reglages::etat_reglages,
            reglages::definir_demarrage_auto,
            reglages::quitter,
            reglages::redemarrer,
            recherche::chercher,
            recherche::ouvrir,
            recherche::chercher_sur_le_web,
            recherche::ouvrir_site,
            recherche::icone,
            contexte::contexte_ouverture,
            contexte::relayer_ctrl_k_ctrl_o,
            apercu::apercu,
            apercu::apercu_pdf,
            bourse::bourse_chercher,
            bourse::bourse_apercu,
        ])
        .setup(move |app| {
            recherche::rafraichir_en_fond(app.handle());
            surveillance::demarrer(app.handle());
            raccourci::demarrer(app.handle());
            touche_windows::demarrer(app.handle());
            // Seulement depuis l'AppImage installée : en développement, le bureau n'est pas touché.
            if demarrage::appimage().is_some() {
                std::thread::spawn(preparer_le_bureau);
            }
            if !en_fond {
                app.state::<contexte::Contexte>().noter_lancement(cible);
                barre::montrer(app.handle());
            }
            Ok(())
        })
        .on_window_event(|fenetre, evenement| {
            // La barre n'est jamais détruite (Alt+F4 compris) : la montrer doit rester instantané.
            if let WindowEvent::CloseRequested { api, .. } = evenement {
                if fenetre.label() == barre::ETIQUETTE {
                    api.prevent_close();
                    let _ = fenetre.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("échec au démarrage de l'application Tauri")
        .run(|app, evenement| {
            // La barre s'arrête (Quitter) : le menu Windows de Cinnamon est rendu.
            if matches!(evenement, tauri::RunEvent::Exit) && demarrage::appimage().is_some() {
                menu_windows::rendre();
            }
            // Fermer les réglages ne quitte pas : seul le bouton « Quitter » arrête le programme.
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = evenement
            {
                if app.get_webview_window(barre::ETIQUETTE).is_some() {
                    api.prevent_exit();
                }
            }
        });
}

/// Au démarrage depuis l'AppImage : la touche Windows est à la barre (le menu de Cinnamon est vidé de
/// son raccourci), et le démarrage avec la session passe par le service s'il y est déjà ou si
/// l'ancien fichier `.desktop` est là.
fn preparer_le_bureau() {
    menu_windows::liberer();
    if let (Some(appimage), Some(unites)) = (demarrage::appimage(), service::dossier_unites()) {
        let autostart = demarrage::dossier_autostart();
        service::mettre_a_jour(&unites, &appimage, autostart.as_deref());
    }
}

/// Notification du bureau quand la barre plante en boucle (le service s'est arrêté).
fn prevenir_du_plantage() {
    let _ = std::process::Command::new("notify-send")
        .args([
            "--app-name=Barre de recherche",
            "La barre de recherche s'arrête en boucle",
            "Le menu Windows est rendu. Relance-la depuis le menu des applications.",
        ])
        .status();
}
