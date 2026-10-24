//! La petite fenêtre de la barre : créée cachée au démarrage, montrée au milieu de l'écran où est la souris.

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

/// Étiquette de la fenêtre dans `tauri.conf.json` et les capacités.
pub const ETIQUETTE: &str = "barre";
/// Largeur en pixels logiques (spécification §6) : la liste (640) et le panneau d'aperçu (460),
/// toujours la même, que l'aperçu ait quelque chose à montrer ou non.
pub const LARGEUR: f64 = 1100.0;
/// Hauteur au plus, quand la liste est pleine.
pub const HAUTEUR_MAX: f64 = 720.0;
const HAUTEUR_MIN: f64 = 40.0;
/// Haut de la barre, en part de la hauteur de l'écran.
const DEPUIS_LE_HAUT: f64 = 0.18;

/// Zone d'un écran, en pixels physiques.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ecran {
    pub x: i32,
    pub y: i32,
    pub largeur: u32,
    pub hauteur: u32,
}

/// Coin haut gauche de la barre : centrée en largeur, à 18 % du haut de l'écran.
pub fn position(ecran: Ecran, largeur_barre: u32) -> (i32, i32) {
    let x = ecran.x + (ecran.largeur as i32 - largeur_barre as i32) / 2;
    let y = ecran.y + (f64::from(ecran.hauteur) * DEPUIS_LE_HAUT).round() as i32;
    (x, y)
}

/// Hauteur demandée par la page, bornée.
pub fn hauteur_bornee(hauteur: f64) -> f64 {
    if hauteur.is_finite() {
        hauteur.clamp(HAUTEUR_MIN, HAUTEUR_MAX)
    } else {
        HAUTEUR_MIN
    }
}

/// Place la barre sur l'écran de la souris, la montre et lui donne le clavier.
pub fn montrer(app: &AppHandle) {
    montrer_a(app, None);
}

/// Comme `montrer`, avec l'heure X11 de l'appui sur le raccourci : le gestionnaire de fenêtres
/// refuse le clavier à une fenêtre qui le demande sans heure récente, quand une autre l'a déjà.
pub fn montrer_a(app: &AppHandle, heure_appui: Option<u32>) {
    let Some(fenetre) = app.get_webview_window(ETIQUETTE) else {
        return;
    };
    let moniteur = app
        .cursor_position()
        .ok()
        .and_then(|souris| app.monitor_from_point(souris.x, souris.y).ok().flatten())
        .or_else(|| fenetre.primary_monitor().ok().flatten());
    if let Some(moniteur) = moniteur {
        let zone = Ecran {
            x: moniteur.position().x,
            y: moniteur.position().y,
            largeur: moniteur.size().width,
            hauteur: moniteur.size().height,
        };
        let largeur = (LARGEUR * moniteur.scale_factor()).round() as u32;
        let (x, y) = position(zone, largeur);
        let _ = fenetre.set_position(PhysicalPosition::new(x, y));
    }
    let _ = fenetre.show();
    match (heure_appui, fenetre.gtk_window()) {
        (Some(heure), Ok(gtk)) => {
            use gtk::prelude::GtkWindowExt;
            gtk.present_with_time(heure);
        }
        _ => {
            let _ = fenetre.set_focus();
        }
    }
    // Une application installée depuis la dernière ouverture apparaît à la suivante.
    crate::recherche::rafraichir_en_fond(app);
}

/// Redonne le clavier à la barre si elle est visible (fin de la prise du raccourci).
pub fn redonner_clavier(app: &AppHandle, heure: u32) {
    let Some(fenetre) = app.get_webview_window(ETIQUETTE) else {
        return;
    };
    if !fenetre.is_visible().unwrap_or(false) {
        return;
    }
    if let Ok(gtk) = fenetre.gtk_window() {
        use gtk::prelude::GtkWindowExt;
        gtk.present_with_time(heure);
    }
}

/// La page mesure son contenu et demande la hauteur qui va avec ; la largeur ne change pas.
/// La fenêtre est déclarée redimensionnable (`tauri.conf.json`) : sinon GTK ignore la demande et
/// la cale à 200 px de haut. Si la largeur n'était pas la bonne, la fenêtre est recentrée.
#[tauri::command]
pub fn ajuster_taille(fenetre: WebviewWindow, hauteur: f64) {
    let largeur = LARGEUR;
    let echelle = fenetre.scale_factor().unwrap_or(1.0);
    let largeur_physique = (largeur * echelle).round() as u32;
    let change_de_largeur = fenetre
        .inner_size()
        .is_ok_and(|t| t.width.abs_diff(largeur_physique) > 1);
    let _ = fenetre.set_size(LogicalSize::new(largeur, hauteur_bornee(hauteur)));
    if change_de_largeur {
        if let (Ok(Some(moniteur)), Ok(actuelle)) =
            (fenetre.current_monitor(), fenetre.outer_position())
        {
            let x = moniteur.position().x
                + (moniteur.size().width as i32 - largeur_physique as i32) / 2;
            let _ = fenetre.set_position(PhysicalPosition::new(x, actuelle.y));
        }
    }
}

/// Échap, clic ailleurs, choix : la page se remet à zéro puis demande à être cachée.
#[tauri::command]
pub fn cacher_barre(fenetre: WebviewWindow) {
    let _ = fenetre.hide();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centree_a_18_pour_cent_du_haut() {
        let ecran = Ecran {
            x: 0,
            y: 0,
            largeur: 1920,
            hauteur: 1080,
        };
        assert_eq!(position(ecran, 640), (640, 194));
    }

    #[test]
    fn sur_le_second_ecran() {
        let ecran = Ecran {
            x: 1920,
            y: 120,
            largeur: 2560,
            hauteur: 1440,
        };
        assert_eq!(position(ecran, 1280), (1920 + 640, 120 + 259));
    }

    #[test]
    fn hauteur_entre_les_bornes() {
        assert_eq!(hauteur_bornee(10.0), HAUTEUR_MIN);
        assert_eq!(hauteur_bornee(120.0), 120.0);
        assert_eq!(hauteur_bornee(5000.0), HAUTEUR_MAX);
        assert_eq!(hauteur_bornee(f64::NAN), HAUTEUR_MIN);
    }
}
