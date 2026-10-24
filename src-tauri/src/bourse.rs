//! Cible `@bourse` (spécification §4, J10) : cours des actions et portefeuille, lus sur l'API bourse
//! d'Eloi. Seule exception à la règle 3 : une seule adresse, réglée dans le `.env` du projet (non
//! versionné) et intégrée à la construction, appelée seulement quand `@bourse` est posé. La
//! liste des actions suivies est lue puis filtrée ici : le texte tapé ne part jamais, seul le
//! symbole d'une action trouvée.

use crate::floue;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Attente au plus pour chaque lecture.
const DELAI: Duration = Duration::from_secs(4);
/// Attente au plus pour se connecter : court, pour passer vite à l'adresse de secours.
const DELAI_CONNEXION: Duration = Duration::from_millis(1500);
/// Rang de la dernière adresse qui a répondu : essayée en premier la fois suivante.
static ADRESSE_PREFEREE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Réponse lue au plus (une année de cours tient en quelques dizaines de Ko).
const TAILLE_MAX: u64 = 4 * 1024 * 1024;
/// Actions proposées au plus pour une saisie.
const MAXIMUM_ACTIONS: usize = 8;
/// Durée de vie des réponses gardées : liste des actions, cours du moment (l'API garde ceux de
/// Yahoo 5 minutes), historique et portefeuille.
const VIE_INSTRUMENTS: Duration = Duration::from_secs(600);
const VIE_COTATIONS: Duration = Duration::from_secs(60);
/// Les barres de 15 min sont refaites par l'API toutes les 15 min.
const VIE_COURS: Duration = Duration::from_secs(60);
const VIE_POSITIONS: Duration = Duration::from_secs(60);

/// Une adresse de l'API : `hote:port` et début du chemin des routes, sans `/` final (souvent vide).
#[derive(Debug, Clone, PartialEq)]
struct Adresse {
    hote: String,
    prefixe: String,
}

/// Réglage pris dans le `.env` du projet à la construction (`build.rs`) :
/// `BOURSE_ADRESSE=http://hote:port`, `BOURSE_ADRESSE_SECOURS` facultative (la même API par un
/// autre chemin réseau, Tailscale par exemple) et `BOURSE_CLE=…`.
#[derive(Debug, PartialEq)]
struct Reglage {
    /// L'adresse, puis celle de secours s'il y en a une.
    adresses: Vec<Adresse>,
    cle: String,
}

fn lire_reglage() -> Result<Reglage, String> {
    analyser_reglage(
        option_env!("BOURSE_ADRESSE").unwrap_or_default(),
        option_env!("BOURSE_ADRESSE_SECOURS").unwrap_or_default(),
        option_env!("BOURSE_CLE").unwrap_or_default(),
    )
}

fn propre(texte: &str) -> bool {
    !texte.is_empty() && texte.chars().all(|c| c.is_ascii_graphic())
}

fn analyser_adresse(adresse: &str, nom: &str) -> Result<Adresse, String> {
    let reste = adresse
        .trim()
        .strip_prefix("http://")
        .ok_or(format!("{nom} doit commencer par http://"))?;
    let (hote, chemin) = reste.split_once('/').unwrap_or((reste, ""));
    if !propre(hote) || !(chemin.is_empty() || propre(chemin)) {
        return Err(format!("{nom} invalide."));
    }
    let hote = if hote.contains(':') {
        hote.to_string()
    } else {
        format!("{hote}:80")
    };
    let prefixe = format!("/{chemin}").trim_end_matches('/').to_string();
    Ok(Adresse { hote, prefixe })
}

fn analyser_reglage(adresse: &str, secours: &str, cle: &str) -> Result<Reglage, String> {
    if adresse.trim().is_empty() {
        return Err("BOURSE_ADRESSE absente du .env du projet (puis npm run installer).".into());
    }
    let mut adresses = vec![analyser_adresse(adresse, "BOURSE_ADRESSE")?];
    if !secours.trim().is_empty() {
        adresses.push(analyser_adresse(secours, "BOURSE_ADRESSE_SECOURS")?);
    }
    let cle = cle.trim().to_string();
    if cle.is_empty() {
        return Err("BOURSE_CLE absente du .env du projet (puis npm run installer).".into());
    }
    if !propre(&cle) {
        return Err("BOURSE_CLE invalide.".into());
    }
    Ok(Reglage { adresses, cle })
}

/// Connexion à la première adresse qui répond, en commençant par celle qui a répondu la
/// dernière fois.
fn connecter(reglage: &Reglage) -> Result<(TcpStream, &Adresse), String> {
    let nombre = reglage.adresses.len();
    let preferee = ADRESSE_PREFEREE.load(std::sync::atomic::Ordering::Relaxed) % nombre.max(1);
    let mut echecs = Vec::new();
    for rang in (0..nombre).map(|i| (preferee + i) % nombre) {
        let adresse = &reglage.adresses[rang];
        let essai = adresse
            .hote
            .to_socket_addrs()
            .map_err(|e| e.to_string())
            .and_then(|mut a| a.next().ok_or("adresse introuvable".to_string()))
            .and_then(|a| {
                TcpStream::connect_timeout(&a, DELAI_CONNEXION).map_err(|e| e.to_string())
            });
        match essai {
            Ok(flux) => {
                ADRESSE_PREFEREE.store(rang, std::sync::atomic::Ordering::Relaxed);
                return Ok((flux, adresse));
            }
            Err(erreur) => echecs.push(format!("{} : {erreur}", adresse.hote)),
        }
    }
    Err(format!("API bourse injoignable ({}).", echecs.join(" ; ")))
}

/// Symbole dans un chemin ou une requête : tout sauf les lettres, chiffres et `-._~` est encodé.
fn encoder(texte: &str) -> String {
    texte
        .bytes()
        .map(|o| match o {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (o as char).to_string()
            }
            _ => format!("%{o:02X}"),
        })
        .collect()
}

/// `GET` sur l'API, réponse JSON. HTTP/1.1 écrit à la main : l'API est en clair sur le réseau
/// local, et la barre n'embarque pas de client HTTP pour une seule API.
fn obtenir(reglage: &Reglage, route: &str) -> Result<Value, String> {
    let (mut flux, adresse) = connecter(reglage)?;
    let injoignable =
        |e: std::io::Error| format!("API bourse injoignable ({}) : {e}", adresse.hote);
    flux.set_read_timeout(Some(DELAI)).map_err(injoignable)?;
    flux.set_write_timeout(Some(DELAI)).map_err(injoignable)?;
    let requete = format!(
        "GET {}{route} HTTP/1.1\r\nHost: {}\r\nX-API-Key: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        adresse.prefixe, adresse.hote, reglage.cle
    );
    flux.write_all(requete.as_bytes()).map_err(injoignable)?;
    let mut octets = Vec::new();
    flux.take(TAILLE_MAX)
        .read_to_end(&mut octets)
        .map_err(injoignable)?;
    let (statut, corps) = analyser_reponse(&octets)?;
    let json: Value = serde_json::from_slice(&corps).unwrap_or(Value::Null);
    match statut {
        200 => Ok(json),
        401 => Err("Clé de l'API bourse refusée (BOURSE_CLE dans le .env du projet).".into()),
        _ => Err(json["erreur"]
            .as_str()
            .map(|e| format!("API bourse : {e}"))
            .unwrap_or(format!("API bourse : erreur {statut}"))),
    }
}

/// Statut et corps d'une réponse HTTP/1.1, corps découpé (`chunked`) recollé.
fn analyser_reponse(octets: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let invalide = || "Réponse de l'API bourse illisible.".to_string();
    let fin = octets
        .windows(4)
        .position(|f| f == b"\r\n\r\n")
        .ok_or_else(invalide)?;
    let entetes = std::str::from_utf8(&octets[..fin]).map_err(|_| invalide())?;
    let mut lignes = entetes.split("\r\n");
    let statut: u16 = lignes
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(invalide)?;
    let mut decoupe = false;
    let mut longueur = None;
    for ligne in lignes {
        let Some((nom, valeur)) = ligne.split_once(':') else {
            continue;
        };
        let valeur = valeur.trim();
        if nom.eq_ignore_ascii_case("transfer-encoding") {
            decoupe = valeur.to_ascii_lowercase().contains("chunked");
        } else if nom.eq_ignore_ascii_case("content-length") {
            longueur = valeur.parse::<usize>().ok();
        }
    }
    let corps = &octets[fin + 4..];
    if decoupe {
        return recoller(corps).ok_or_else(invalide).map(|c| (statut, c));
    }
    let corps = match longueur {
        Some(n) if n <= corps.len() => &corps[..n],
        _ => corps,
    };
    Ok((statut, corps.to_vec()))
}

/// Corps `Transfer-Encoding: chunked` : morceaux précédés de leur taille en hexadécimal.
fn recoller(mut reste: &[u8]) -> Option<Vec<u8>> {
    let mut corps = Vec::new();
    loop {
        let fin = reste.windows(2).position(|f| f == b"\r\n")?;
        let taille = std::str::from_utf8(&reste[..fin]).ok()?;
        let taille = usize::from_str_radix(taille.split(';').next()?.trim(), 16).ok()?;
        reste = &reste[fin + 2..];
        if taille == 0 {
            return Some(corps);
        }
        corps.extend_from_slice(reste.get(..taille)?);
        reste = reste.get(taille + 2..)?;
    }
}

/// Liste d'objets d'une réponse : un tableau, le tableau du champ `champ`, ou un objet dont chaque
/// clé est un symbole.
fn objets(json: &Value, champ: &str) -> Vec<Value> {
    match json {
        Value::Array(liste) => liste.clone(),
        Value::Object(objet) => match objet.get(champ) {
            Some(Value::Array(liste)) => liste.clone(),
            Some(interieur @ Value::Object(_)) => objets(interieur, champ),
            _ => objet
                .iter()
                .filter_map(|(cle, valeur)| {
                    let mut valeur = valeur.as_object()?.clone();
                    valeur
                        .entry("symbole")
                        .or_insert_with(|| Value::String(cle.clone()));
                    Some(Value::Object(valeur))
                })
                .collect(),
        },
        _ => Vec::new(),
    }
}

fn texte(valeur: &Value, champ: &str) -> Option<String> {
    valeur[champ]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(String::from)
}

#[derive(Debug, Clone)]
struct Instrument {
    symbole: String,
    nom: String,
    devise: String,
    /// Le symbole, cherché aussi (« ALO.PA »).
    mots_cles: Vec<String>,
}

fn instruments_de(json: &Value) -> Vec<Instrument> {
    objets(json, "instruments")
        .iter()
        .filter_map(|i| {
            let symbole = texte(i, "symbole")?;
            Some(Instrument {
                nom: texte(i, "nom").unwrap_or_else(|| symbole.clone()),
                devise: texte(i, "devise").unwrap_or_default(),
                mots_cles: vec![symbole.clone()],
                symbole,
            })
        })
        .collect()
}

/// Cours du moment d'un symbole (`/cotations`).
#[derive(Debug, Clone, PartialEq)]
struct Cotation {
    prix: Option<f64>,
    /// Écart avec la clôture de la veille, dans la devise, et en %.
    variation: Option<f64>,
    variation_pct: Option<f64>,
    devise: Option<String>,
    ouvert: bool,
}

fn cotations_de(json: &Value) -> HashMap<String, Cotation> {
    objets(json, "cotations")
        .iter()
        .filter_map(|c| {
            Some((
                texte(c, "symbole")?,
                Cotation {
                    prix: c["prix"].as_f64(),
                    variation: c["variation"].as_f64(),
                    variation_pct: c["variation_pct"].as_f64(),
                    devise: texte(c, "devise"),
                    ouvert: c["etat_marche"].as_str() == Some("REGULAR"),
                },
            ))
        })
        .collect()
}

/// Une ligne de la liste avec `@bourse`, telle que la page la reçoit.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LigneBourse {
    /// `portefeuille`, `action:<symbole>` ou `position:<rang>`.
    id: String,
    titre: String,
    /// Symbole, compte, quantité, état du marché.
    detail: String,
    /// Cours (action) ou valeur en euros (position, portefeuille).
    valeur: Option<f64>,
    devise: String,
    /// Variation du jour, en %.
    variation_pct: Option<f64>,
    /// Argent gagné ou perdu aujourd'hui, dans `devise` (par action pour une action).
    gain: Option<f64>,
    /// Lettres trouvées dans le titre.
    positions: Vec<usize>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Point {
    /// Début de la barre de 15 min, en UTC (`2026-10-09T07:00:00.000Z`), ou `maintenant`.
    heure: String,
    valeur: f64,
}

/// Aperçu d'une ligne `@bourse`, à la façon de l'ancien Google Finance : la valeur, ce qui a été
/// gagné ou perdu dans la journée, et la courbe de cette journée.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApercuBourse {
    titre: String,
    details: String,
    valeur: Option<f64>,
    devise: String,
    variation_pct: Option<f64>,
    gain: Option<f64>,
    /// Jour de la courbe (UTC, `2026-10-09`) : la dernière séance, aujourd'hui ou avant.
    jour: Option<String>,
    points: Vec<Point>,
}

/// Gain du jour d'une valeur qui a bougé de `pct` % depuis la veille.
fn gain_du_jour(valeur: f64, pct: f64) -> f64 {
    valeur - valeur / (1.0 + pct / 100.0)
}

/// Lignes du portefeuille (`/positions`) : le total d'abord, puis chaque position.
fn lignes_du_portefeuille(json: &Value) -> Vec<LigneBourse> {
    let totaux = &json["totaux"];
    let positions = objets(json, "positions");
    let gains: Vec<f64> = positions
        .iter()
        .filter_map(|p| {
            Some(gain_du_jour(
                p["valeur_eur"].as_f64()?,
                p["variations"]["jour"].as_f64()?,
            ))
        })
        .collect();
    // Gain du jour du portefeuille : somme de celui de chaque position.
    let du_jour = (!gains.is_empty()).then(|| gains.iter().sum::<f64>());
    let valeur = totaux["valeur"].as_f64();
    let mut lignes = vec![LigneBourse {
        id: "portefeuille".into(),
        titre: "Portefeuille".into(),
        detail: format!(
            "{} position{}{}",
            positions.len(),
            if positions.len() > 1 { "s" } else { "" },
            if totaux["incomplet"].as_bool() == Some(true) {
                ", cours manquant"
            } else {
                ""
            }
        ),
        valeur,
        devise: "EUR".into(),
        variation_pct: valeur
            .zip(du_jour)
            .filter(|(v, g)| (v - g).abs() > f64::EPSILON)
            .map(|(v, g)| g / (v - g) * 100.0),
        gain: du_jour,
        positions: Vec::new(),
    }];
    lignes.extend(positions.iter().enumerate().map(|(rang, p)| {
        let symbole = texte(p, "symbole").unwrap_or_default();
        let quantite = p["quantite"].as_f64().map(nombre).unwrap_or_default();
        let valeur = p["valeur_eur"].as_f64();
        let pct = p["variations"]["jour"].as_f64();
        LigneBourse {
            id: format!("position:{rang}"),
            titre: texte(p, "nom").unwrap_or_else(|| symbole.clone()),
            detail: [
                symbole.as_str(),
                &texte(p, "compte").unwrap_or_default(),
                &quantite,
            ]
            .into_iter()
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
            valeur,
            devise: "EUR".into(),
            variation_pct: pct,
            gain: valeur.zip(pct).map(|(v, p)| gain_du_jour(v, p)),
            positions: Vec::new(),
        }
    }));
    lignes
}

/// Nombre sans décimales inutiles (« 20 », « 0,5 »).
fn nombre(n: f64) -> String {
    let t = format!("{n:.4}");
    t.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}

/// Ligne d'une action suivie, avec son cours du moment.
fn ligne_d_une_action(
    instrument: &Instrument,
    cotation: Option<&Cotation>,
    positions: Vec<usize>,
) -> LigneBourse {
    let etat = match cotation {
        Some(c) if c.ouvert => "marché ouvert",
        Some(_) => "marché fermé",
        None => "pas de cours",
    };
    let prix = cotation.and_then(|c| c.prix);
    let pct = cotation.and_then(|c| c.variation_pct);
    LigneBourse {
        id: format!("action:{}", instrument.symbole),
        titre: instrument.nom.clone(),
        detail: format!("{} · {etat}", instrument.symbole),
        valeur: prix,
        devise: cotation
            .and_then(|c| c.devise.clone())
            .unwrap_or_else(|| instrument.devise.clone()),
        variation_pct: pct,
        gain: cotation
            .and_then(|c| c.variation)
            .or_else(|| prix.zip(pct).map(|(v, p)| gain_du_jour(v, p))),
        positions,
    }
}

/// Actions suivies qui correspondent à la saisie, avec leur cours du moment.
fn lignes_des_actions(
    saisie: &str,
    instruments: &[Instrument],
    cotations: &HashMap<String, Cotation>,
) -> Vec<LigneBourse> {
    choisir(saisie, instruments)
        .into_iter()
        .map(|(instrument, positions)| {
            ligne_d_une_action(instrument, cotations.get(&instrument.symbole), positions)
        })
        .collect()
}

fn choisir<'a>(saisie: &str, instruments: &'a [Instrument]) -> Vec<(&'a Instrument, Vec<usize>)> {
    floue::classer_avec_mots_cles(
        saisie,
        instruments,
        |i| &i.nom,
        |i| &i.mots_cles,
        MAXIMUM_ACTIONS,
    )
    .into_iter()
    .map(|(i, c)| (i, c.positions))
    .collect()
}

/// Date du jour en UTC (`2026-10-11`), sans bibliothèque de dates.
fn aujourd_hui() -> String {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    date_du_jour(secondes / 86_400)
}

/// Date du `jours`-ième jour après le 1er janvier 1970 (algorithme de H. Hinnant).
fn date_du_jour(jours: u64) -> String {
    let z = jours + 719_468;
    let ere = z / 146_097;
    let jour_de_l_ere = z % 146_097;
    let annee_de_l_ere = (jour_de_l_ere - jour_de_l_ere / 1460 + jour_de_l_ere / 36_524
        - jour_de_l_ere / 146_096)
        / 365;
    let jour_de_l_annee =
        jour_de_l_ere - (365 * annee_de_l_ere + annee_de_l_ere / 4 - annee_de_l_ere / 100);
    let mp = (5 * jour_de_l_annee + 2) / 153;
    let jour = jour_de_l_annee - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = annee_de_l_ere + ere * 400 + u64::from(mois <= 2);
    format!("{annee:04}-{mois:02}-{jour:02}")
}

/// Barres de 15 min de la dernière journée de la liste (celle de la dernière barre), et ce jour.
fn points_du_dernier_jour(liste: &[Value], champ: &str) -> (Option<String>, Vec<Point>) {
    let points: Vec<Point> = liste
        .iter()
        .filter_map(|p| {
            Some(Point {
                heure: texte(p, "horodatage")?,
                valeur: p[champ].as_f64()?,
            })
        })
        .collect();
    let Some(jour) = points
        .last()
        .and_then(|p| p.heure.get(..10))
        .map(String::from)
    else {
        return (None, Vec::new());
    };
    let points = points
        .into_iter()
        .filter(|p| p.heure.starts_with(&jour))
        .collect();
    (Some(jour), points)
}

fn a_l_echelle(points: Vec<Point>, facteur: f64) -> Vec<Point> {
    points
        .into_iter()
        .map(|p| Point {
            valeur: p.valeur * facteur,
            ..p
        })
        .collect()
}

/// Aperçu d'une ligne : ses chiffres du jour et la courbe ; marché ouvert, la courbe finit sur
/// le cours du moment.
fn apercu_de(
    ligne: LigneBourse,
    (jour, mut points): (Option<String>, Vec<Point>),
    cours_du_moment: Option<f64>,
) -> ApercuBourse {
    if let Some(prix) = cours_du_moment {
        points.push(Point {
            heure: "maintenant".into(),
            valeur: prix,
        });
    }
    ApercuBourse {
        titre: ligne.titre,
        details: ligne.detail,
        valeur: ligne.valeur,
        devise: ligne.devise,
        variation_pct: ligne.variation_pct,
        gain: ligne.gain,
        jour,
        points,
    }
}

/// Réponses gardées un moment, pour ne pas rappeler l'API à chaque frappe.
#[derive(Default)]
pub struct Bourse {
    instruments: Mutex<Option<(Instant, Vec<Instrument>)>>,
    cotations: Mutex<HashMap<String, (Instant, Option<Cotation>)>>,
    cours: Mutex<HashMap<String, (Instant, Value)>>,
    positions: Mutex<Option<(Instant, Value)>>,
}

fn frais<T: Clone>(garde: &Mutex<Option<(Instant, T)>>, vie: Duration) -> Option<T> {
    let garde = garde.lock().ok()?;
    garde
        .as_ref()
        .filter(|(quand, _)| quand.elapsed() < vie)
        .map(|(_, v)| v.clone())
}

impl Bourse {
    fn instruments(&self, reglage: &Reglage) -> Result<Vec<Instrument>, String> {
        if let Some(liste) = frais(&self.instruments, VIE_INSTRUMENTS) {
            return Ok(liste);
        }
        let liste = instruments_de(&obtenir(reglage, "/instruments")?);
        if let Ok(mut garde) = self.instruments.lock() {
            *garde = Some((Instant::now(), liste.clone()));
        }
        Ok(liste)
    }

    fn positions(&self, reglage: &Reglage) -> Result<Value, String> {
        if let Some(json) = frais(&self.positions, VIE_POSITIONS) {
            return Ok(json);
        }
        let json = obtenir(reglage, "/positions")?;
        if let Ok(mut garde) = self.positions.lock() {
            *garde = Some((Instant::now(), json.clone()));
        }
        Ok(json)
    }

    /// Cours du moment des symboles, ceux qui manquent demandés ensemble.
    fn cotations(
        &self,
        reglage: &Reglage,
        symboles: &[&str],
    ) -> Result<HashMap<String, Cotation>, String> {
        let mut connues = HashMap::new();
        let mut a_demander = Vec::new();
        if let Ok(garde) = self.cotations.lock() {
            for symbole in symboles {
                match garde.get(*symbole) {
                    Some((quand, cotation)) if quand.elapsed() < VIE_COTATIONS => {
                        if let Some(c) = cotation {
                            connues.insert(symbole.to_string(), c.clone());
                        }
                    }
                    _ => a_demander.push(*symbole),
                }
            }
        }
        if !a_demander.is_empty() {
            let liste: Vec<String> = a_demander.iter().map(|s| encoder(s)).collect();
            let recues = cotations_de(&obtenir(
                reglage,
                &format!("/cotations?symboles={}", liste.join(",")),
            )?);
            if let Ok(mut garde) = self.cotations.lock() {
                for symbole in a_demander {
                    // Inconnu de Yahoo : absent de la réponse, gardé comme tel.
                    let cotation = recues.get(symbole).cloned();
                    garde.insert(symbole.to_string(), (Instant::now(), cotation));
                }
            }
            connues.extend(recues);
        }
        Ok(connues)
    }

    /// Barres de 15 min d'une route, gardées une minute (`/valorisation/intraday` prend
    /// environ 2 s à l'API).
    fn journee(&self, reglage: &Reglage, route: &str) -> Result<Value, String> {
        if let Ok(garde) = self.cours.lock() {
            if let Some((quand, json)) = garde.get(route) {
                if quand.elapsed() < VIE_COURS {
                    return Ok(json.clone());
                }
            }
        }
        let json = obtenir(reglage, route)?;
        if let Ok(mut garde) = self.cours.lock() {
            garde.insert(route.to_string(), (Instant::now(), json.clone()));
        }
        Ok(json)
    }

    fn cours(&self, reglage: &Reglage, symbole: &str) -> Result<Value, String> {
        // Par défaut, les 5 derniers jours : la dernière séance y est, week-end compris.
        self.journee(
            reglage,
            &format!("/cours/{}/intraday?intervalle=15m", encoder(symbole)),
        )
    }

    fn chercher(&self, saisie: &str) -> Result<Vec<LigneBourse>, String> {
        let reglage = lire_reglage()?;
        if saisie.trim().is_empty() {
            return Ok(lignes_du_portefeuille(&self.positions(&reglage)?));
        }
        let instruments = self.instruments(&reglage)?;
        let symboles: Vec<&str> = choisir(saisie, &instruments)
            .iter()
            .map(|(i, _)| i.symbole.as_str())
            .collect();
        let cotations = if symboles.is_empty() {
            HashMap::new()
        } else {
            self.cotations(&reglage, &symboles)?
        };
        Ok(lignes_des_actions(saisie, &instruments, &cotations))
    }

    fn apercu(&self, id: &str) -> Result<ApercuBourse, String> {
        let reglage = lire_reglage()?;
        type Journee = (Option<String>, Vec<Point>);
        let cours_du_moment = |symbole: &str| -> Result<(Option<Cotation>, Journee), String> {
            let cotation = self.cotations(&reglage, &[symbole])?.remove(symbole);
            let cours = self.cours(&reglage, symbole)?;
            Ok((
                cotation,
                points_du_dernier_jour(&objets(&cours, "cours"), "cloture"),
            ))
        };
        if id == "portefeuille" {
            let positions = self.positions(&reglage)?;
            // La courbe du portefeuille tourne en continu (cryptos) : la journée suffit.
            let valorisation = self.journee(
                &reglage,
                &format!("/valorisation/intraday?intervalle=15m&du={}", aujourd_hui()),
            )?;
            let ligne = lignes_du_portefeuille(&positions).swap_remove(0);
            let journee = points_du_dernier_jour(&objets(&valorisation, "points"), "valeur");
            return Ok(apercu_de(ligne, journee, None));
        }
        if let Some(rang) = id.strip_prefix("position:") {
            let rang: usize = rang.parse().map_err(|_| "Position inconnue.")?;
            let positions = self.positions(&reglage)?;
            let symbole = objets(&positions, "positions")
                .get(rang)
                .and_then(|p| texte(p, "symbole"))
                .ok_or("Position inconnue.")?;
            let ligne = lignes_du_portefeuille(&positions)
                .into_iter()
                .nth(rang + 1)
                .ok_or("Position inconnue.")?;
            // Courbe du cours de l'action ramenée à la valeur de la position en euros (quantité
            // et change d'aujourd'hui), pour être à l'échelle de la valeur et du gain affichés.
            let (cotation, (jour, points)) = cours_du_moment(&symbole)?;
            let maintenant = cotation.filter(|c| c.ouvert).and_then(|c| c.prix);
            let dernier = maintenant.or(points.last().map(|p| p.valeur));
            let facteur = ligne
                .valeur
                .zip(dernier)
                .filter(|(_, d)| *d != 0.0)
                .map(|(v, d)| v / d);
            let Some(facteur) = facteur else {
                return Ok(apercu_de(ligne, (jour, Vec::new()), None));
            };
            let points = a_l_echelle(points, facteur);
            return Ok(apercu_de(
                ligne,
                (jour, points),
                maintenant.map(|p| p * facteur),
            ));
        }
        let symbole = id.strip_prefix("action:").ok_or("Ligne inconnue.")?;
        // Seulement une action suivie : rien d'autre ne part vers l'API.
        let instrument = self
            .instruments(&reglage)?
            .into_iter()
            .find(|i| i.symbole == symbole)
            .ok_or("Action non suivie.")?;
        let (cotation, journee) = cours_du_moment(symbole)?;
        let ligne = ligne_d_une_action(&instrument, cotation.as_ref(), Vec::new());
        let maintenant = cotation.filter(|c| c.ouvert).and_then(|c| c.prix);
        Ok(apercu_de(ligne, journee, maintenant))
    }
}

/// Lignes de `@bourse` : le portefeuille au champ vide, sinon les actions suivies qui
/// correspondent à la saisie.
#[tauri::command]
pub async fn bourse_chercher(app: AppHandle, saisie: String) -> Result<Vec<LigneBourse>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Bourse>().chercher(&saisie))
        .await
        .map_err(|e| e.to_string())?
}

/// Aperçu d'une ligne de `@bourse` : valeur, gain du jour, courbe de la journée.
#[tauri::command]
pub async fn bourse_apercu(app: AppHandle, id: String) -> Result<ApercuBourse, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Bourse>().apercu(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reglage_lu_et_verifie() {
        let r = analyser_reglage("http://192.0.2.10:8140", "", "abc").unwrap();
        assert_eq!(
            r,
            Reglage {
                adresses: vec![Adresse {
                    hote: "192.0.2.10:8140".into(),
                    prefixe: String::new()
                }],
                cle: "abc".into()
            }
        );
        let r = analyser_reglage("http://pi.local/api/", "http://198.51.100.7:8140", "x").unwrap();
        assert_eq!(
            (r.adresses[0].hote.as_str(), r.adresses[0].prefixe.as_str()),
            ("pi.local:80", "/api")
        );
        assert_eq!(r.adresses[1].hote, "198.51.100.7:8140");
        assert!(analyser_reglage("https://pi:8140", "", "x").is_err());
        assert!(analyser_reglage("http://pi:8140", "", "").is_err());
        assert!(analyser_reglage("", "", "x").is_err());
        assert!(analyser_reglage("http://pi:8140", "198.51.100.7", "x").is_err());
        assert!(analyser_reglage("http://pi:8140", "", "a\r\nX: y").is_err());
        assert!(analyser_reglage("http://pi 8140", "", "x").is_err());
    }

    /// L'adresse qui ne répond pas est passée, celle de secours sert, et devient la préférée.
    #[test]
    fn adresse_de_secours() {
        let serveur = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = serveur.local_addr().unwrap().port();
        // Port fermé : refus immédiat, sans attendre le délai.
        let ferme = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port_ferme = ferme.local_addr().unwrap().port();
        drop(ferme);
        let reglage = analyser_reglage(
            &format!("http://127.0.0.1:{port_ferme}"),
            &format!("http://127.0.0.1:{port}"),
            "x",
        )
        .unwrap();
        let (_, adresse) = connecter(&reglage).unwrap();
        assert_eq!(adresse.hote, format!("127.0.0.1:{port}"));
        assert_eq!(
            ADRESSE_PREFEREE.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        drop(serveur);
        let erreur = connecter(&reglage).unwrap_err();
        assert!(erreur.contains("injoignable"), "{erreur}");
        ADRESSE_PREFEREE.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn symboles_encodes() {
        assert_eq!(encoder("^FCHI"), "%5EFCHI");
        assert_eq!(encoder("EURUSD=X"), "EURUSD%3DX");
        assert_eq!(encoder("ALO.PA"), "ALO.PA");
        assert_eq!(encoder("a/../b c"), "a%2F..%2Fb%20c");
    }

    #[test]
    fn reponses_http() {
        let simple = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n[1]\nrest";
        assert_eq!(analyser_reponse(simple).unwrap(), (200, b"[1]\n".to_vec()));
        let decoupee =
            b"HTTP/1.1 404 Not Found\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n{\"a\r\n2;x\r\n\":\r\n2\r\n1}\r\n0\r\n\r\n";
        assert_eq!(
            analyser_reponse(decoupee).unwrap(),
            (404, b"{\"a\":1}".to_vec())
        );
        assert!(analyser_reponse(b"pas http").is_err());
    }

    fn instruments() -> Vec<Instrument> {
        instruments_de(&json!([
            { "symbole": "ALO.PA", "nom": "Alstom", "type": "EQUITY", "devise": "EUR" },
            { "symbole": "CW8.PA", "nom": "Amundi MSCI World", "type": "ETF", "devise": "EUR" },
            { "symbole": "AAPL", "nom": "Apple Inc.", "type": "EQUITY", "devise": "USD" },
        ]))
    }

    #[test]
    fn actions_trouvees_par_nom_ou_symbole() {
        let liste = instruments();
        let cotations = cotations_de(&json!({ "cotations": [
            { "symbole": "ALO.PA", "prix": 21.5, "variation_pct": 1.2, "devise": "EUR", "etat_marche": "REGULAR" }
        ]}));
        let lignes = lignes_des_actions("alstom", &liste, &cotations);
        assert_eq!(lignes.len(), 1);
        assert_eq!(lignes[0].id, "action:ALO.PA");
        assert_eq!(lignes[0].valeur, Some(21.5));
        assert_eq!(lignes[0].detail, "ALO.PA · marché ouvert");
        let par_symbole = lignes_des_actions("aapl", &liste, &HashMap::new());
        assert_eq!(par_symbole[0].titre, "Apple Inc.");
        assert_eq!(par_symbole[0].valeur, None);
        assert_eq!(par_symbole[0].devise, "USD");
    }

    #[test]
    fn cotations_sous_plusieurs_formes() {
        let par_cle = cotations_de(&json!({ "ALO.PA": { "prix": 21.5, "etat_marche": "CLOSED" } }));
        assert_eq!(par_cle["ALO.PA"].prix, Some(21.5));
        assert!(!par_cle["ALO.PA"].ouvert);
        let tableau = cotations_de(&json!([{ "symbole": "AAPL", "prix": 180.0 }]));
        assert_eq!(tableau["AAPL"].prix, Some(180.0));
    }

    #[test]
    fn portefeuille_total_puis_positions() {
        let json = json!({
            "jour": "2026-10-11",
            "totaux": { "valeur": 10500, "investi": 9000, "rendement_pct": 18.89 },
            "positions": [{
                "compte": "PEA", "symbole": "CW8.PA", "nom": "Amundi MSCI World", "quantite": 20,
                "valeur_eur": 10500, "rendement_pct": 16.67, "variations": { "jour": 5.0 }
            }]
        });
        let lignes = lignes_du_portefeuille(&json);
        assert_eq!(lignes.len(), 2);
        assert_eq!(lignes[0].titre, "Portefeuille");
        assert_eq!(lignes[0].detail, "1 position");
        assert!((lignes[0].variation_pct.unwrap() - 5.0).abs() < 1e-9);
        assert!((lignes[0].gain.unwrap() - 500.0).abs() < 1e-9);
        assert_eq!(lignes[1].detail, "CW8.PA · PEA · 20");
        assert_eq!(lignes[1].id, "position:0");
        assert!((lignes[1].gain.unwrap() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn apercu_avec_le_cours_du_moment() {
        let liste = instruments();
        let cotation = Cotation {
            prix: Some(21.5),
            variation: Some(0.3),
            variation_pct: Some(1.42),
            devise: Some("EUR".into()),
            ouvert: true,
        };
        let ligne = ligne_d_une_action(&liste[0], Some(&cotation), Vec::new());
        assert_eq!(ligne.gain, Some(0.3));
        // Vendredi puis lundi : seule la dernière journée reste.
        let barres: Vec<Value> = [
            "2026-10-09T07:00:00.000Z",
            "2026-10-09T15:15:00.000Z",
            "2026-10-12T07:00:00.000Z",
            "2026-10-12T07:15:00.000Z",
        ]
        .iter()
        .map(|h| json!({ "horodatage": h, "cloture": 21.0 }))
        .collect();
        let apercu = apercu_de(
            ligne,
            points_du_dernier_jour(&barres, "cloture"),
            Some(21.5),
        );
        assert_eq!(apercu.titre, "Alstom");
        assert_eq!(apercu.jour.as_deref(), Some("2026-10-12"));
        assert_eq!(apercu.points.len(), 3);
        assert_eq!(apercu.points[0].heure, "2026-10-12T07:00:00.000Z");
        assert_eq!(apercu.points.last().unwrap().heure, "maintenant");
        assert_eq!(points_du_dernier_jour(&[], "cloture"), (None, Vec::new()));
        let sans_variation = Cotation {
            variation: None,
            ..cotation
        };
        let gain = ligne_d_une_action(&liste[0], Some(&sans_variation), Vec::new())
            .gain
            .unwrap();
        assert!((gain - (21.5 - 21.5 / 1.0142)).abs() < 1e-9);
    }

    #[test]
    fn dates_sans_bibliotheque() {
        assert_eq!(date_du_jour(0), "1970-01-01");
        assert_eq!(date_du_jour(20_737), "2026-10-11");
        assert_eq!(date_du_jour(11_016), "2000-02-29");
    }

    #[test]
    fn nombres_sans_zeros() {
        assert_eq!(nombre(20.0), "20");
        assert_eq!(nombre(0.5), "0,5");
        assert_eq!(nombre(1.23456), "1,2346");
    }

    /// Vraie requête : `BOURSE_ADRESSE=http://hote:port cargo test sante -- --ignored`.
    #[test]
    #[ignore = "réseau : demande BOURSE_ADRESSE"]
    fn sante_de_l_api() {
        let adresse = std::env::var("BOURSE_ADRESSE").expect("BOURSE_ADRESSE");
        let reglage = analyser_reglage(&adresse, "", "essai").unwrap();
        assert_eq!(obtenir(&reglage, "/health").unwrap()["ok"], true);
        let refus = obtenir(&reglage, "/instruments").unwrap_err();
        assert!(refus.contains("refusée"), "{refus}");
    }

    /// Journée d'une action et du portefeuille sur la vraie API (réglage du projet) :
    /// `cargo test journee_reelle -- --ignored --nocapture`.
    #[test]
    #[ignore = "réseau : vraie API bourse"]
    fn journee_reelle() {
        let bourse = Bourse::default();
        for id in ["action:ALO.PA", "portefeuille", "portefeuille"] {
            let debut = Instant::now();
            let apercu = bourse.apercu(id).unwrap();
            println!("{id} : {:?}", debut.elapsed());
            let (premier, dernier) = (
                apercu.points.first().unwrap(),
                apercu.points.last().unwrap(),
            );
            println!(
                "{id} : jour {:?}, {} points, de {} à {}",
                apercu.jour,
                apercu.points.len(),
                premier.heure,
                dernier.heure
            );
            assert!(apercu.points.len() > 2);
        }
    }
}
