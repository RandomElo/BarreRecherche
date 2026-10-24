//! Tient l'index des fichiers à jour (spécification §11 tranché : parcours au démarrage puis inotify) :
//! chaque dossier indexé est surveillé ; un avis de création, suppression ou renommage fait relire
//! le dossier concerné. Les avis sont regroupés sur un court délai, une rafale ne coûte qu'une lecture.

use crate::recherche::Index;
use crate::sources::fichiers::IndexFichiers;
use notify::event::{EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Attente après un premier avis, pour regrouper ceux qui suivent.
const REGROUPEMENT: Duration = Duration::from_millis(150);
/// Au-delà de cette part d'éléments retirés, l'index est reconstruit (mémoire rendue).
const PART_DE_MORTS_MAXIMUM: f64 = 0.3;

/// Construit l'index du dossier personnel puis le surveille, dans un fil à part.
pub fn demarrer(app: &AppHandle) {
    let Some(maison) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("fichiers".into())
        .spawn(move || surveiller(&app, &maison));
}

fn surveiller(app: &AppHandle, maison: &Path) {
    let (envoi, reception) = mpsc::channel();
    let mut veilleur = match RecommendedWatcher::new(envoi, notify::Config::default()) {
        Ok(veilleur) => Some(veilleur),
        Err(erreur) => {
            eprintln!("Surveillance des fichiers impossible : {erreur}");
            None
        }
    };
    let index = app.state::<Index>();
    let construire = |veilleur: &mut Option<RecommendedWatcher>| {
        // Le coffre de Notes est cherché à part (pages par leur titre), pas comme des fichiers.
        let exclus = crate::sources::notes::coffre().into_iter().collect();
        let fichiers = IndexFichiers::construire_sans(maison, exclus);
        if let Some(veilleur) = veilleur.as_mut() {
            for dossier in fichiers.dossiers() {
                let _ = veilleur.watch(&dossier, RecursiveMode::NonRecursive);
            }
        }
        index.remplacer_fichiers(fichiers);
    };
    construire(&mut veilleur);
    if veilleur.is_none() {
        return;
    }

    while let Ok(premier) = reception.recv() {
        let mut avis = vec![premier];
        std::thread::sleep(REGROUPEMENT);
        avis.extend(reception.try_iter());

        let mut a_relire = BTreeSet::new();
        let mut tout_relire = false;
        for resultat in avis {
            let Ok(evenement) = resultat else {
                // Avis perdus (file pleine) : seul un parcours complet est sûr.
                tout_relire = true;
                continue;
            };
            if evenement.need_rescan() {
                tout_relire = true;
            }
            let utile = matches!(
                evenement.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(_))
                    | EventKind::Any
                    | EventKind::Other
            );
            if utile {
                for chemin in evenement.paths {
                    if let Some(parent) = chemin.parent() {
                        a_relire.insert(parent.to_path_buf());
                    }
                }
            }
        }

        if tout_relire || index.part_de_morts_des_fichiers() > PART_DE_MORTS_MAXIMUM {
            construire(&mut veilleur);
            continue;
        }
        let ajoutes = index.resynchroniser_fichiers(&a_relire);
        if let Some(veilleur) = veilleur.as_mut() {
            for dossier in ajoutes {
                let _ = veilleur.watch(&dossier, RecursiveMode::NonRecursive);
            }
        }
    }
}
