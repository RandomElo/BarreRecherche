//! Raccourci global `Ctrl+K`, pris directement sur le serveur X11 (`XGrabKey` par x11rb) : un fil
//! attend l'appui sans délai. Le greffon global-shortcut de Tauri ne lit les touches que toutes les
//! 50 ms, ce qui mangeait l'essentiel du budget de 50 ms (spécification §10, J1). La prise est synchrone :
//! le clavier attend la décision, qui ouvre la barre avec la cible du logiciel au premier plan ou
//! rend la touche à ce logiciel (`contexte.rs`, spécification §2). `Ctrl` tenu après `K`, la barre attend :
//! `Ctrl` relâché seul l'ouvre, une autre touche est renvoyée au logiciel avec `Ctrl+K` devant.

use crate::cibles::Cible;
use crate::contexte::{self, Contexte, Decision, SYMBOLE_CTRL, SYMBOLE_CTRL_DROIT, SYMBOLE_K};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{Allow, ConnectionExt, GrabMode, GrabStatus, ModMask, Window};
use x11rb::protocol::Event;
use x11rb::CURRENT_TIME;

/// Libellé montré dans la barre et les réglages.
pub const LIBELLE: &str = "Ctrl+K";

/// Dans VS Code, après `Ctrl+K` et `Ctrl` relâché : temps laissé à la seconde touche d'un accord
/// (`Ctrl+K` puis `Ctrl+O`, `Ctrl` relâché entre les deux, comme le fait une touche macro) avant
/// d'ouvrir la barre.
const DELAI_ACCORD: Duration = Duration::from_millis(300);

/// Le raccourci a-t-il pu être pris ? Faux quand un autre programme a déjà ces touches.
#[derive(Default)]
pub struct EtatRaccourci {
    actif: AtomicBool,
}

impl EtatRaccourci {
    pub fn actif(&self) -> bool {
        self.actif.load(Ordering::Relaxed)
    }
}

/// Modificateurs à prendre : Ctrl, avec et sans Verr. Maj et Verr. Num, qui ne doivent pas
/// empêcher le raccourci de marcher.
fn combinaisons() -> [ModMask; 4] {
    let ctrl = ModMask::CONTROL;
    [
        ctrl,
        ctrl | ModMask::LOCK,
        ctrl | ModMask::M2,
        ctrl | ModMask::LOCK | ModMask::M2,
    ]
}

/// Code de touche qui donne `symbole` sans modificateur, d'après la table du clavier
/// (`symboles_par_touche` symboles par code, à partir de `premier_code`).
pub fn code_de_touche(
    symbole: u32,
    premier_code: u8,
    symboles_par_touche: u8,
    table: &[u32],
) -> Option<u8> {
    if symboles_par_touche == 0 {
        return None;
    }
    table
        .chunks(usize::from(symboles_par_touche))
        .position(|symboles| symboles.first() == Some(&symbole))
        .and_then(|rang| u8::try_from(usize::from(premier_code) + rang).ok())
}

/// Table du clavier lue sur le serveur X11.
pub struct TableClavier {
    premier: u8,
    symboles_par_touche: u8,
    symboles: Vec<u32>,
}

impl TableClavier {
    pub fn code(&self, symbole: u32) -> Option<u8> {
        code_de_touche(
            symbole,
            self.premier,
            self.symboles_par_touche,
            &self.symboles,
        )
    }
}

pub fn table_du_clavier(connexion: &impl Connection) -> Result<TableClavier, String> {
    let (premier, dernier) = (connexion.setup().min_keycode, connexion.setup().max_keycode);
    let table = connexion
        .get_keyboard_mapping(premier, dernier - premier + 1)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    Ok(TableClavier {
        premier,
        symboles_par_touche: table.keysyms_per_keycode,
        symboles: table.keysyms,
    })
}

/// Prend le raccourci et lance le fil qui l'écoute. Note dans l'état s'il a pu être pris.
pub fn demarrer(app: &AppHandle) {
    let actif = match prendre_et_ecouter(app.clone()) {
        Ok(()) => true,
        Err(erreur) => {
            eprintln!("{LIBELLE} n'a pas pu être pris : {erreur}");
            false
        }
    };
    app.state::<EtatRaccourci>()
        .actif
        .store(actif, Ordering::Relaxed);
}

fn prendre_et_ecouter(app: AppHandle) -> Result<(), String> {
    let (connexion, numero_ecran) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let racine = connexion.setup().roots[numero_ecran].root;
    let code = table_du_clavier(&connexion)?
        .code(SYMBOLE_K)
        .ok_or("la touche K est introuvable sur ce clavier")?;

    for (rang, modificateurs) in combinaisons().into_iter().enumerate() {
        // Clavier en mode synchrone : figé à l'appui jusqu'à `allow_events`, pour pouvoir rendre
        // la touche au logiciel (`REPLAY_KEYBOARD`) au lieu de la garder.
        let reponse = connexion
            .grab_key(
                false,
                racine,
                modificateurs,
                code,
                GrabMode::ASYNC,
                GrabMode::SYNC,
            )
            .map_err(|e| e.to_string())?
            .check();
        if let Err(erreur) = reponse {
            // Rendre ce qui a déjà été pris, pour ne pas garder un raccourci à moitié.
            for deja in &combinaisons()[..rang] {
                let _ = connexion.ungrab_key(code, racine, *deja);
            }
            let _ = connexion.flush();
            return Err(match erreur {
                ReplyError::X11Error(_) => "déjà pris par un autre programme".to_string(),
                autre => autre.to_string(),
            });
        }
    }

    let table = table_du_clavier(&connexion)?;
    let ctrls: Vec<u8> = [SYMBOLE_CTRL, SYMBOLE_CTRL_DROIT]
        .iter()
        .filter_map(|&s| table.code(s))
        .collect();
    let modificateurs = touches_de_modificateurs(&connexion);

    std::thread::Builder::new()
        .name("raccourci".into())
        .spawn(move || {
            let mut attente: Option<Attente> = None;
            loop {
                // En attente d'un accord, l'échéance passée sans touche ouvre la barre.
                let echeance = attente.as_ref().and_then(|a| a.echeance);
                let evenement = match echeance {
                    Some(echeance) => match attendre_jusqu_a(&connexion, echeance) {
                        Ok(Some(evenement)) => evenement,
                        Ok(None) => {
                            eprintln!("[raccourci] pas de seconde touche : la barre s'ouvre");
                            let _ = connexion.ungrab_keyboard(CURRENT_TIME);
                            let _ = connexion.flush();
                            let Attente { cible, fenetre, heure, .. } =
                                attente.take().expect("en cours");
                            ouvrir_barre_et_redonner_le_clavier(&app, cible, fenetre, heure);
                            continue;
                        }
                        Err(_) => break,
                    },
                    None => match connexion.wait_for_event() {
                        Ok(evenement) => evenement,
                        Err(_) => break,
                    },
                };
                if let Some(en_cours) = attente.as_mut() {
                    match evenement {
                        Event::KeyPress(touche)
                            if en_cours.seconde.is_none()
                                && !modificateurs.contains(&touche.detail) =>
                        {
                            en_cours.seconde = Some(touche.detail);
                            en_cours.avec_ctrl = u16::from(touche.state) & u16::from(ModMask::CONTROL) != 0;
                            eprintln!(
                                "[raccourci] touche suivante pressée (code {}, Ctrl {})",
                                touche.detail, en_cours.avec_ctrl
                            );
                        }
                        Event::KeyRelease(relachee)
                            if Some(relachee.detail) == en_cours.seconde =>
                        {
                            // Une autre touche après Ctrl+K : tout est rendu au logiciel.
                            eprintln!("[raccourci] touche suivante relâchée : Ctrl+K et elle rendus au logiciel");
                            let _ = connexion.ungrab_keyboard(CURRENT_TIME);
                            let avec_ctrl = en_cours.avec_ctrl;
                            attente = None;
                            if let Err(erreur) = contexte::envoyer_ctrl_k_puis(
                                &app,
                                &connexion,
                                racine,
                                &table,
                                relachee.detail,
                                avec_ctrl,
                            ) {
                                eprintln!("Ctrl+K n'a pas pu être rendu : {erreur}");
                            }
                        }
                        Event::KeyRelease(relachee)
                            if ctrls.contains(&relachee.detail)
                                && en_cours.seconde.is_none()
                                && en_cours.echeance.is_none() =>
                        {
                            en_cours.heure = relachee.time;
                            if en_cours.cible == Some(Cible::Vsc) {
                                // VS Code : un accord peut suivre, `Ctrl` relâché entre les deux.
                                eprintln!("[raccourci] Ctrl relâché : attente d'une seconde touche");
                                en_cours.echeance = Some(Instant::now() + DELAI_ACCORD);
                            } else {
                                // Ctrl relâché seul : la barre s'ouvre.
                                eprintln!("[raccourci] Ctrl relâché seul : la barre s'ouvre");
                                let _ = connexion.ungrab_keyboard(CURRENT_TIME);
                                let _ = connexion.flush();
                                let Attente { cible, fenetre, heure, .. } =
                                    attente.take().expect("en cours");
                                ouvrir_barre_et_redonner_le_clavier(&app, cible, fenetre, heure);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                // K relâché : fin de la prise du clavier. Tant qu'elle dure, GTK ne donne pas le
                // clavier à la barre quand une autre fenêtre l'avait : il faut le redemander.
                if let Event::KeyRelease(relachee) = &evenement {
                    if relachee.detail == code {
                        let heure = relachee.time;
                        let pour_la_barre = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            crate::barre::redonner_clavier(&pour_la_barre, heure);
                        });
                    }
                    continue;
                }
                let Event::KeyPress(touche) = evenement else {
                    continue;
                };
                if touche.detail != code {
                    continue;
                }
                let contexte_app = app.state::<Contexte>();
                let (decision, fenetre) = if contexte_app.prendre_relais() {
                    (Decision::Laisser, None)
                } else {
                    contexte::decider_pour(&connexion, racine)
                };
                if decision == Decision::Laisser {
                    eprintln!("[raccourci] Ctrl+K rendu au logiciel (relais)");
                }
                let mode = match decision {
                    Decision::Laisser => Allow::REPLAY_KEYBOARD,
                    Decision::Barre(_) => Allow::ASYNC_KEYBOARD,
                };
                // Toujours libérer le clavier, aussitôt.
                let _ = connexion.allow_events(mode, CURRENT_TIME);
                let _ = connexion.flush();
                let Decision::Barre(cible) = decision else {
                    continue;
                };
                let deja_ouverte = app
                    .get_webview_window(crate::barre::ETIQUETTE)
                    .is_some_and(|f| f.is_visible().unwrap_or(false));
                // Barre fermée et Ctrl encore tenu (ou VS Code, où un accord peut suivre) :
                // attendre de savoir si une touche suit. Le clavier est pris en entier, pour voir
                // les touches suivantes sans qu'elles partent au logiciel.
                let ctrl_tenu = ctrls
                    .iter()
                    .any(|&c| contexte::touche_enfoncee(&connexion, c));
                let accord_possible = cible == Some(Cible::Vsc);
                let mut attendre = !deja_ouverte
                    && (ctrl_tenu || accord_possible)
                    && prendre_le_clavier(&connexion, racine);
                // Ctrl relâché avant ou pendant la prise : ce relâchement ne nous parviendra
                // pas. Dans VS Code, l'échéance de l'accord court dès maintenant ; ailleurs, la
                // barre s'ouvre tout de suite (le clavier resterait pris sinon).
                let mut echeance = None;
                if attendre
                    && !ctrls
                        .iter()
                        .any(|&c| contexte::touche_enfoncee(&connexion, c))
                {
                    if accord_possible {
                        echeance = Some(Instant::now() + DELAI_ACCORD);
                    } else {
                        let _ = connexion.ungrab_keyboard(CURRENT_TIME);
                        let _ = connexion.flush();
                        attendre = false;
                    }
                }
                // Trace de chaque Ctrl+K, pour comprendre un raccourci rendu ou pris à tort (barre
                // lancée depuis un terminal ; la sortie d'erreur ne va nulle part sinon).
                eprintln!(
                    "[raccourci] Ctrl+K : {decision:?}, barre déjà ouverte {deja_ouverte}, Ctrl tenu {ctrl_tenu}, attente de la suite {attendre}"
                );
                if attendre {
                    attente = Some(Attente {
                        cible,
                        fenetre,
                        seconde: None,
                        avec_ctrl: true,
                        echeance,
                        heure: touche.time,
                    });
                } else {
                    ouvrir_barre(&app, cible, fenetre, touche.time);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// `Ctrl+K` pressé, `Ctrl` encore tenu (ou VS Code) : la barre attend la suite.
struct Attente {
    cible: Option<Cible>,
    fenetre: Option<Window>,
    /// Touche pressée après `K`, renvoyée au logiciel quand elle est relâchée.
    seconde: Option<u8>,
    /// `Ctrl` était enfoncé avec la seconde touche (`Ctrl+K Ctrl+O`, pas `Ctrl+K V`).
    avec_ctrl: bool,
    /// `Ctrl` relâché dans VS Code : sans seconde touche d'ici là, la barre s'ouvre.
    echeance: Option<Instant>,
    /// Heure X11 du dernier appui ou relâchement, pour donner le clavier à la barre.
    heure: u32,
}

/// Prochain événement, ou `None` si `echeance` passe avant.
fn attendre_jusqu_a(
    connexion: &impl Connection,
    echeance: Instant,
) -> Result<Option<Event>, x11rb::errors::ConnectionError> {
    loop {
        if let Some(evenement) = connexion.poll_for_event()? {
            return Ok(Some(evenement));
        }
        if Instant::now() >= echeance {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(3));
    }
}

/// Ouvre la barre à la fin de l'attente d'une seconde touche. Juste après la fin de la prise du
/// clavier, le gestionnaire de fenêtres le laisse à l'ancienne fenêtre : le redemander un instant
/// après.
fn ouvrir_barre_et_redonner_le_clavier(
    app: &AppHandle,
    cible: Option<Cible>,
    fenetre: Option<Window>,
    heure: u32,
) {
    ouvrir_barre(app, cible, fenetre, heure);
    let pour_la_barre = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        let encore = pour_la_barre.clone();
        let _ = pour_la_barre.run_on_main_thread(move || {
            crate::barre::redonner_clavier(&encore, heure);
        });
    });
}

/// Montre la barre avec la cible du logiciel d'où vient `Ctrl+K` ; déjà ouverte, elle reste comme
/// elle est.
fn ouvrir_barre(app: &AppHandle, cible: Option<Cible>, fenetre: Option<Window>, heure: u32) {
    let pour_la_barre = app.clone();
    let _ = app.run_on_main_thread(move || {
        let deja_ouverte = pour_la_barre
            .get_webview_window(crate::barre::ETIQUETTE)
            .is_some_and(|f| f.is_visible().unwrap_or(false));
        if !deja_ouverte {
            pour_la_barre.state::<Contexte>().noter(cible, fenetre);
        }
        crate::barre::montrer_a(&pour_la_barre, Some(heure));
    });
}

fn prendre_le_clavier(connexion: &impl Connection, racine: Window) -> bool {
    connexion
        .grab_keyboard(
            false,
            racine,
            CURRENT_TIME,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|r| r.status == GrabStatus::SUCCESS)
}

/// Codes des touches de modificateurs (Maj, Ctrl, Alt…) : tenues après `Ctrl+K`, elles ne sont
/// pas « la touche suivante ».
fn touches_de_modificateurs(connexion: &impl Connection) -> Vec<u8> {
    connexion
        .get_modifier_mapping()
        .ok()
        .and_then(|c| c.reply().ok())
        .map(|r| r.keycodes.into_iter().filter(|&c| c != 0).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trouve_le_code_de_la_touche() {
        // Deux symboles par touche, codes 8, 9, 10 : « j J », « k K », « l L ».
        let table = [0x6a, 0x4a, 0x6b, 0x4b, 0x6c, 0x4c];
        assert_eq!(code_de_touche(SYMBOLE_K, 8, 2, &table), Some(9));
    }

    #[test]
    fn ignore_le_symbole_en_majuscule() {
        let table = [0x4b, 0x6b];
        assert_eq!(code_de_touche(SYMBOLE_K, 8, 2, &table), None);
    }

    #[test]
    fn table_vide_ou_invalide() {
        assert_eq!(code_de_touche(SYMBOLE_K, 8, 0, &[0x6b]), None);
        assert_eq!(code_de_touche(SYMBOLE_K, 8, 2, &[]), None);
    }
}
