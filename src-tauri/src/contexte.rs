//! `Ctrl+K` contextuel (spécification §2) : le logiciel au premier plan (`_NET_ACTIVE_WINDOW` puis
//! `WM_CLASS`) décide de la cible posée d'office. Depuis VS Code, `Ctrl+O` dans la barre rend
//! `Ctrl+K Ctrl+O` à VS Code (XTest).

use crate::cibles::Cible;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;

/// Symboles X11 (XK_Control_L, XK_Control_R, XK_k, XK_o).
pub const SYMBOLE_CTRL: u32 = 0xffe3;
pub const SYMBOLE_CTRL_DROIT: u32 = 0xffe4;
pub const SYMBOLE_K: u32 = 0x6b;
const SYMBOLE_O: u32 = 0x6f;

/// Ce que `Ctrl+K` fait selon le logiciel au premier plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Ouvrir la barre, avec cette cible posée s'il y en a une.
    Barre(Option<Cible>),
    /// Rendre `Ctrl+K` au logiciel : c'est le `Ctrl+K` que la barre envoie elle-même (relais).
    Laisser,
}

/// Décision d'après la classe X11 de la fenêtre active (`WM_CLASS` : instance, classe).
pub fn decider(instance: &str, classe: &str) -> Decision {
    let instance = instance.to_lowercase();
    let classe = classe.to_lowercase();
    let un_des =
        |noms: &[&str]| noms.contains(&instance.as_str()) || noms.contains(&classe.as_str());
    if un_des(&["code", "code-oss", "vscodium", "com.microsoft.vscode"]) {
        Decision::Barre(Some(Cible::Vsc))
    } else if ["ghostty", "gnome-terminal"]
        .iter()
        .any(|nom| instance.contains(nom) || classe.contains(nom))
    {
        Decision::Barre(Some(Cible::Terminal))
    } else if un_des(&["firefox", "firefox-esr"]) {
        Decision::Barre(Some(Cible::Firefox))
    } else if un_des(&["notes", "fr.eloi.notes"]) {
        Decision::Barre(Some(Cible::Notes))
    } else {
        Decision::Barre(None)
    }
}

/// `WM_CLASS` brut (« instance\0classe\0 ») en ses deux parties.
pub fn lire_wm_class(octets: &[u8]) -> (String, String) {
    let mut parties = octets
        .split(|&o| o == 0)
        .map(|p| String::from_utf8_lossy(p).into_owned());
    let instance = parties.next().unwrap_or_default();
    let classe = parties.next().unwrap_or_default();
    (instance, classe)
}

/// Fenêtre au premier plan, d'après le gestionnaire de fenêtres.
pub fn fenetre_active(connexion: &impl Connection, racine: Window) -> Option<Window> {
    let atome = connexion
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .ok()?
        .reply()
        .ok()?
        .atom;
    let valeur = connexion
        .get_property(false, racine, atome, AtomEnum::WINDOW, 0, 1)
        .ok()?
        .reply()
        .ok()?;
    let fenetre = valeur.value32()?.next();
    fenetre.filter(|&f| f != 0)
}

/// Décision pour la fenêtre active ; sans fenêtre active ou sans classe, barre sans cible.
pub fn decider_pour(connexion: &impl Connection, racine: Window) -> (Decision, Option<Window>) {
    let Some(fenetre) = fenetre_active(connexion, racine) else {
        return (Decision::Barre(None), None);
    };
    let classe = connexion
        .get_property(false, fenetre, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
        .ok()
        .and_then(|c| c.reply().ok());
    let decision = match classe {
        Some(c) => {
            let (instance, classe) = lire_wm_class(&c.value);
            decider(&instance, &classe)
        }
        None => Decision::Barre(None),
    };
    (decision, Some(fenetre))
}

/// Ouverture de la barre telle que la page la reçoit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ouverture {
    /// Nom de la cible posée d'office (`vsc`…), sans le `@`.
    cible: Option<&'static str>,
    /// Ouverte depuis VS Code : `Ctrl+O` rend `Ctrl+K Ctrl+O` à VS Code.
    relais: bool,
}

/// Dernière ouverture par `Ctrl+K`, et relais de `Ctrl+K Ctrl+O` en cours.
#[derive(Default)]
pub struct Contexte {
    /// Lue une fois par la page quand la barre prend le focus.
    en_attente: Mutex<Option<Ouverture>>,
    /// Fenêtre de VS Code d'où vient la dernière ouverture.
    fenetre_vsc: Mutex<Option<Window>>,
    /// `Ctrl+K` envoyé par la barre elle-même, à rendre au logiciel : le raccourci ne doit pas
    /// le reprendre. Périmé au bout d'une seconde, pour ne jamais avaler un vrai `Ctrl+K`.
    /// Nombre de `Ctrl+K` à rendre (deux pour `Ctrl+K Ctrl+K`).
    relais_depuis: Mutex<Option<(Instant, u8)>>,
}

impl Contexte {
    /// Note l'ouverture par `Ctrl+K` depuis `fenetre`, avec la cible `cible`.
    pub fn noter(&self, cible: Option<Cible>, fenetre: Option<Window>) {
        let depuis_vsc = cible == Some(Cible::Vsc);
        if let Ok(mut attente) = self.en_attente.lock() {
            *attente = Some(Ouverture {
                cible: cible.map(Cible::nom),
                relais: depuis_vsc,
            });
        }
        if let Ok(mut vsc) = self.fenetre_vsc.lock() {
            *vsc = fenetre.filter(|_| depuis_vsc);
        }
    }

    /// Vrai (une seule fois) si ce `Ctrl+K` est celui que la barre vient d'envoyer.
    pub fn prendre_relais(&self) -> bool {
        let Ok(mut relais) = self.relais_depuis.lock() else {
            return false;
        };
        match relais.take() {
            Some((depuis, restants)) if depuis.elapsed() < Duration::from_secs(1) => {
                if restants > 1 {
                    *relais = Some((depuis, restants - 1));
                }
                true
            }
            _ => false,
        }
    }

    /// Les `combien` prochains `Ctrl+K` viennent de la barre : à rendre au logiciel.
    fn armer_relais(&self, combien: u8) {
        if let Ok(mut relais) = self.relais_depuis.lock() {
            *relais = Some((Instant::now(), combien));
        }
    }
}

/// Ce que la page doit poser en s'ouvrant : la cible du logiciel d'où vient `Ctrl+K`, une fois.
#[tauri::command]
pub fn contexte_ouverture(contexte: tauri::State<'_, Contexte>) -> Option<Ouverture> {
    contexte.en_attente.lock().ok().and_then(|mut a| a.take())
}

/// `Ctrl+O` dans la barre ouverte depuis VS Code : cache la barre, rend le premier plan à VS Code
/// et lui envoie `Ctrl+K Ctrl+O` (spécification §2).
#[tauri::command]
pub fn relayer_ctrl_k_ctrl_o(app: AppHandle) -> Result<(), String> {
    let contexte = app.state::<Contexte>();
    let fenetre = contexte
        .fenetre_vsc
        .lock()
        .ok()
        .and_then(|f| *f)
        .ok_or("La barre n'a pas été ouverte depuis VS Code.")?;
    if let Some(barre) = app.get_webview_window(crate::barre::ETIQUETTE) {
        let _ = barre.hide();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(erreur) = relayer(&app, fenetre) {
            eprintln!("Ctrl+K Ctrl+O n'a pas pu être rendu à VS Code : {erreur}");
        }
    });
    Ok(())
}

fn relayer(app: &AppHandle, fenetre: Window) -> Result<(), String> {
    let (connexion, numero_ecran) = x11rb::connect(None).map_err(|e| e.to_string())?;
    let racine = connexion.setup().roots[numero_ecran].root;
    let atome = connexion
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .atom;
    // Demande au gestionnaire de fenêtres de rendre le premier plan à VS Code (source 2 : outil
    // de l'utilisateur, toujours écouté), puis attend qu'il l'ait fait.
    let message = ClientMessageEvent::new(32, fenetre, atome, [2, 0, 0, 0, 0]);
    connexion
        .send_event(
            false,
            racine,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        )
        .map_err(|e| e.to_string())?;
    connexion.flush().map_err(|e| e.to_string())?;
    let limite = Instant::now() + Duration::from_millis(500);
    while fenetre_active(&connexion, racine) != Some(fenetre) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(10));
    }
    // Attend que Ctrl et O soient relâchés : le serveur X ignore l'appui simulé d'une touche
    // encore enfoncée sur le vrai clavier. Le délai laisse aussi VS Code prendre le premier plan.
    let limite = Instant::now() + Duration::from_millis(1500);
    while touches_enfoncees(&connexion) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(80));
    // Une ligne pour comprendre un relais qui n'aboutit pas (barre lancée depuis un terminal).
    eprintln!(
        "Ctrl+K Ctrl+O vers VS Code : premier plan rendu {}, touches relâchées {}",
        if fenetre_active(&connexion, racine) == Some(fenetre) {
            "oui"
        } else {
            "non"
        },
        if touches_enfoncees(&connexion) {
            "non"
        } else {
            "oui"
        },
    );

    let table = crate::raccourci::table_du_clavier(&connexion)?;
    let o = table
        .code(SYMBOLE_O)
        .ok_or("touche O introuvable sur ce clavier")?;
    envoyer_ctrl_k_puis(app, &connexion, racine, &table, o, true)?;
    // Attend que le serveur ait tout reçu avant de fermer la connexion.
    connexion
        .get_input_focus()
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Envoie `Ctrl+K` puis `Ctrl+<touche>` au logiciel qui a le clavier (XTest). Le `Ctrl+K` envoyé
/// repasse par le raccourci, qui le rend au logiciel (relais armé). `Ctrl` n'est simulé que s'il
/// n'est plus tenu : relâcher un `Ctrl` simulé pendant que le vrai est tenu le ferait oublier.
pub fn envoyer_ctrl_k_puis(
    app: &AppHandle,
    connexion: &impl Connection,
    racine: Window,
    table: &crate::raccourci::TableClavier,
    touche: u8,
    avec_ctrl: bool,
) -> Result<(), String> {
    let code = |symbole| {
        table
            .code(symbole)
            .ok_or("touche introuvable sur ce clavier")
    };
    let (ctrl, k) = (code(SYMBOLE_CTRL)?, code(SYMBOLE_K)?);
    let ctrl_tenu = touche_enfoncee(connexion, ctrl)
        || code(SYMBOLE_CTRL_DROIT).is_ok_and(|droit| touche_enfoncee(connexion, droit));
    app.state::<Contexte>()
        .armer_relais(if touche == k { 2 } else { 1 });
    // `Ctrl` simulé s'il n'est pas tenu : autour de K, et de la seconde touche si elle était
    // pressée avec (`Ctrl+K Ctrl+O`), pas sinon (`Ctrl+K V`).
    let mut suite = Vec::new();
    if !ctrl_tenu {
        suite.push((KEY_PRESS_EVENT, ctrl));
    }
    suite.extend([(KEY_PRESS_EVENT, k), (KEY_RELEASE_EVENT, k)]);
    if !ctrl_tenu && !avec_ctrl {
        suite.push((KEY_RELEASE_EVENT, ctrl));
    }
    suite.extend([(KEY_PRESS_EVENT, touche), (KEY_RELEASE_EVENT, touche)]);
    if !ctrl_tenu && avec_ctrl {
        suite.push((KEY_RELEASE_EVENT, ctrl));
    }
    for (genre, code) in suite {
        connexion
            .xtest_fake_input(genre, code, 0, racine, 0, 0, 0)
            .map_err(|e| e.to_string())?;
    }
    connexion.flush().map_err(|e| e.to_string())?;
    Ok(())
}

/// Cette touche est-elle enfoncée ?
pub fn touche_enfoncee(connexion: &impl Connection, code: u8) -> bool {
    connexion
        .query_keymap()
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|carte| carte.keys[usize::from(code / 8)] & (1 << (code % 8)) != 0)
}

/// Une touche est-elle enfoncée sur le clavier (Verr. Maj et Verr. Num ne comptent pas : ce sont
/// des états, pas des touches tenues) ?
fn touches_enfoncees(connexion: &impl Connection) -> bool {
    connexion
        .query_keymap()
        .ok()
        .and_then(|c| c.reply().ok())
        .is_some_and(|carte| carte.keys.iter().any(|&octet| octet != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cible_selon_le_logiciel() {
        assert_eq!(
            decider("com.microsoft.vscode", "com.microsoft.VSCode"),
            Decision::Barre(Some(Cible::Vsc))
        );
        assert_eq!(decider("code", "Code"), Decision::Barre(Some(Cible::Vsc)));
        assert_eq!(
            decider("ghostty", "com.mitchellh.ghostty"),
            Decision::Barre(Some(Cible::Terminal))
        );
        // Le terminal de Cinnamon, quand Ghostty n'est pas là.
        assert_eq!(
            decider("gnome-terminal-server", "Gnome-terminal"),
            Decision::Barre(Some(Cible::Terminal))
        );
        assert_eq!(
            decider("Navigator", "firefox"),
            Decision::Barre(Some(Cible::Firefox))
        );
        assert_eq!(decider("nemo", "Nemo"), Decision::Barre(None));
        assert_eq!(decider("", ""), Decision::Barre(None));
    }

    #[test]
    fn notes_ouvre_la_barre_sur_ses_pages() {
        // Classe relevée sur la fenêtre de Notes : (« notes », « notes »).
        assert_eq!(
            decider("notes", "notes"),
            Decision::Barre(Some(Cible::Notes))
        );
        assert_eq!(
            decider("notes", "Notes"),
            Decision::Barre(Some(Cible::Notes))
        );
    }

    #[test]
    fn wm_class_en_deux_parties() {
        assert_eq!(
            lire_wm_class(b"Navigator\0firefox\0"),
            ("Navigator".to_string(), "firefox".to_string())
        );
        assert_eq!(lire_wm_class(b""), (String::new(), String::new()));
    }

    #[test]
    fn ouverture_lue_une_seule_fois_et_relais_depuis_vsc() {
        let contexte = Contexte::default();
        contexte.noter(Some(Cible::Vsc), Some(42));
        let ouverture = contexte.en_attente.lock().unwrap().take();
        assert_eq!(
            ouverture,
            Some(Ouverture {
                cible: Some("vsc"),
                relais: true
            })
        );
        assert_eq!(*contexte.fenetre_vsc.lock().unwrap(), Some(42));
        contexte.noter(None, Some(7));
        assert_eq!(*contexte.fenetre_vsc.lock().unwrap(), None);
    }

    #[test]
    fn relais_pris_une_fois_et_perime() {
        let contexte = Contexte::default();
        assert!(!contexte.prendre_relais());
        contexte.armer_relais(1);
        assert!(contexte.prendre_relais());
        assert!(!contexte.prendre_relais());
        contexte.armer_relais(2);
        assert!(contexte.prendre_relais() && contexte.prendre_relais());
        assert!(!contexte.prendre_relais());
        *contexte.relais_depuis.lock().unwrap() =
            Some((Instant::now() - Duration::from_secs(2), 1));
        assert!(!contexte.prendre_relais());
    }
}
