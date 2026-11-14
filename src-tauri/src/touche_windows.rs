//! La touche Windows (Super) seule ouvre la barre, comme `Ctrl+K`. Elle n'est pas prise (`XGrabKey`) :
//! une prise sur Super volerait aussi `Super+E` et les autres raccourcis du bureau. La barre se
//! contente de lire l'état des touches à intervalle régulier, sans rien intercepter, et
//! ouvre quand Super est relâchée sans qu'aucune autre touche ni clic n'ait eu lieu entre-temps.
//!
//! Le menu de Cinnamon ne doit pas s'ouvrir sur Super en même temps :
//! `gsettings set org.cinnamon.muffin overlay-key ''` (et pas de raccourci Super dans l'applet du menu).

use crate::raccourci::{self, TableClavier};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;

const SYMBOLE_SUPER_GAUCHE: u32 = 0xffeb;
const SYMBOLE_SUPER_DROITE: u32 = 0xffec;

/// Ce qui s'est passé au clavier ou à la souris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touche {
    Appui(u8),
    Relache(u8),
    Clic,
}

/// Reconnaît « Super tout seul » : appui puis relâchement sans rien d'autre entre les deux.
pub struct Detecteur {
    supers: Vec<u8>,
    tenue: bool,
    seule: bool,
}

impl Detecteur {
    pub fn new(supers: Vec<u8>) -> Self {
        Self {
            supers,
            tenue: false,
            seule: false,
        }
    }

    /// Vrai quand ce relâchement de Super doit ouvrir la barre.
    pub fn evenement(&mut self, touche: Touche) -> bool {
        match touche {
            Touche::Appui(code) if self.supers.contains(&code) => {
                // Un appui répété par le clavier ne recommence pas la détection.
                if !self.tenue {
                    self.tenue = true;
                    self.seule = true;
                }
                false
            }
            Touche::Appui(_) | Touche::Clic => {
                self.seule = false;
                false
            }
            Touche::Relache(code) if self.supers.contains(&code) => {
                let ouvre = self.tenue && self.seule;
                self.tenue = false;
                self.seule = false;
                ouvre
            }
            Touche::Relache(_) => false,
        }
    }
}

/// Lance le fil qui surveille. Hors X11, la touche Windows n'ouvre simplement rien.
pub fn demarrer(app: &AppHandle) {
    if let Err(erreur) = surveiller(app.clone()) {
        eprintln!("La touche Windows n'a pas pu être surveillée : {erreur}");
    }
}

/// Intervalle de lecture de l'état du clavier : bien plus court qu'un appui de touche.
const PERIODE: Duration = Duration::from_millis(15);
/// Boutons de la souris dans le masque renvoyé par `QueryPointer` (boutons 1 à 5).
const MASQUE_BOUTONS: u16 = 0x1f00;

/// Lit l'état du clavier et de la souris à intervalle régulier plutôt que les événements bruts de
/// XInput2 : sur le poste d'Eloi, ces événements arrivaient en retard ou pas du tout, et la barre
/// ne s'ouvrait qu'un appui sur deux.
fn surveiller(app: AppHandle) -> Result<(), String> {
    let (connexion, numero_ecran) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let racine = connexion.setup().roots[numero_ecran].root;
    let table: TableClavier = raccourci::table_du_clavier(&connexion)?;
    let supers: Vec<u8> = [SYMBOLE_SUPER_GAUCHE, SYMBOLE_SUPER_DROITE]
        .iter()
        .filter_map(|&s| table.code(s))
        .collect();
    if supers.is_empty() {
        return Err("aucune touche Windows sur ce clavier".into());
    }

    std::thread::Builder::new()
        .name("touche-windows".into())
        .spawn(move || {
            let mut detecteur = Detecteur::new(supers);
            let mut touches = [0u8; 32];
            let mut boutons = false;
            loop {
                std::thread::sleep(PERIODE);
                let Some((nouvelles, nouveaux_boutons)) = lire_etat(&connexion, racine) else {
                    continue;
                };
                let mut ouvre = false;
                for code in 8..=255usize {
                    let (avant, apres) = (bit(&touches, code), bit(&nouvelles, code));
                    if apres && !avant {
                        ouvre |= detecteur.evenement(Touche::Appui(code as u8));
                    } else if avant && !apres {
                        ouvre |= detecteur.evenement(Touche::Relache(code as u8));
                    }
                }
                if nouveaux_boutons && !boutons {
                    ouvre |= detecteur.evenement(Touche::Clic);
                }
                touches = nouvelles;
                boutons = nouveaux_boutons;
                if ouvre {
                    let heure = crate::contexte::heure_du_serveur().unwrap_or(0);
                    let pour_la_barre = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        let ouverte = pour_la_barre
                            .get_webview_window(crate::barre::ETIQUETTE)
                            .is_some_and(|f| f.is_visible().unwrap_or(false));
                        if ouverte {
                            // Super seule sur la barre ouverte la ferme (la page se remet à zéro à la perte du focus).
                            if let Some(fenetre) =
                                pour_la_barre.get_webview_window(crate::barre::ETIQUETTE)
                            {
                                let _ = fenetre.hide();
                            }
                        } else {
                            raccourci::ouvrir_barre_et_redonner_le_clavier(
                                &pour_la_barre,
                                None,
                                None,
                                heure,
                            );
                        }
                    });
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn bit(touches: &[u8; 32], code: usize) -> bool {
    touches[code / 8] & (1 << (code % 8)) != 0
}

/// Touches enfoncées et présence d'un bouton de souris enfoncé.
fn lire_etat(connexion: &impl Connection, racine: u32) -> Option<([u8; 32], bool)> {
    let clavier = connexion.query_keymap().ok()?.reply().ok()?;
    let souris = connexion.query_pointer(racine).ok()?.reply().ok()?;
    Some((clavier.keys, u16::from(souris.mask) & MASQUE_BOUTONS != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAUCHE: u8 = 133;
    const DROITE: u8 = 134;
    const A: u8 = 38;

    fn detecteur() -> Detecteur {
        Detecteur::new(vec![GAUCHE, DROITE])
    }

    #[test]
    fn super_seul_ouvre_au_relachement() {
        let mut d = detecteur();
        assert!(!d.evenement(Touche::Appui(GAUCHE)));
        assert!(d.evenement(Touche::Relache(GAUCHE)));
    }

    #[test]
    fn super_droite_ouvre_aussi() {
        let mut d = detecteur();
        d.evenement(Touche::Appui(DROITE));
        assert!(d.evenement(Touche::Relache(DROITE)));
    }

    #[test]
    fn super_avec_une_autre_touche_n_ouvre_pas() {
        let mut d = detecteur();
        d.evenement(Touche::Appui(GAUCHE));
        d.evenement(Touche::Appui(A));
        d.evenement(Touche::Relache(A));
        assert!(!d.evenement(Touche::Relache(GAUCHE)));
    }

    #[test]
    fn super_avec_un_clic_n_ouvre_pas() {
        let mut d = detecteur();
        d.evenement(Touche::Appui(GAUCHE));
        d.evenement(Touche::Clic);
        assert!(!d.evenement(Touche::Relache(GAUCHE)));
    }

    #[test]
    fn appui_repete_ne_change_rien() {
        let mut d = detecteur();
        d.evenement(Touche::Appui(GAUCHE));
        d.evenement(Touche::Appui(GAUCHE));
        assert!(d.evenement(Touche::Relache(GAUCHE)));
    }

    #[test]
    fn relachement_sans_appui_n_ouvre_pas() {
        let mut d = detecteur();
        assert!(!d.evenement(Touche::Relache(GAUCHE)));
    }

    #[test]
    fn la_detection_recommence_apres_chaque_relachement() {
        let mut d = detecteur();
        d.evenement(Touche::Appui(GAUCHE));
        d.evenement(Touche::Appui(A));
        d.evenement(Touche::Relache(GAUCHE));
        d.evenement(Touche::Appui(GAUCHE));
        assert!(d.evenement(Touche::Relache(GAUCHE)));
    }
}
