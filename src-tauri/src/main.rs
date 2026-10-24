// Pas de console en plus de la fenêtre.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    barre_recherche::lancer();
}
