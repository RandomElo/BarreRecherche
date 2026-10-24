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
mod raccourci;
mod recents;
mod recherche;
mod reglages;
mod sources;
mod surveillance;

use tauri::{Manager, WindowEvent};

/// Démarre le programme. Avec `--fond` (démarrage avec la session), la barre reste cachée.
pub fn lancer() {
    let en_fond = std::env::args().any(|argument| argument == "--fond");

    tauri::Builder::default()
        // En premier : un second lancement montre la barre de l'instance déjà là, puis s'arrête.
        .plugin(tauri_plugin_single_instance::init(
            |app, _arguments, _dossier| {
                barre::montrer(app);
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
            if !en_fond {
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
