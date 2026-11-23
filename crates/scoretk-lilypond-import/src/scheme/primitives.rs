// SPDX-License-Identifier: GPL-3.0-only
//
// Copyright (C) 2026 jperon
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3 as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/gpl-3.0.html>.

//! Primitives Scheme (R7RS-small §6) et valeurs LilyPond.
//!
//! Les primitives LilyPond suivent les sources citées dans
//! `docs/agents/lilypond-scheme.md` : `ly:make-moment`
//! (`lily/moment-scheme.cc:28`), `ly:make-duration`
//! (`lily/duration-scheme.cc:53-99`), `magstep` (`scm/lily-library.scm`).
//! Les fonctions de configuration (`ly:set-option`, `ly:expect-warning`,
//! `set-global-staff-size`, `set-paper-size`) rendent `#<unspecified>` : le
//! lecteur ne lit que leur datum (`lily/parser.yy:565-615`).

use scoretk_core::model::Fraction;

use super::datum::{liste, Value};
use super::eval::apply;

/// Valeur exacte : entier quand le dénominateur vaut 1 (R7RS §6.2).
fn exact_value(fraction: Fraction) -> Value {
    if fraction.denominator() == 1 {
        Value::Int(fraction.numerator())
    } else {
        Value::Rational(fraction)
    }
}

fn nombre(valeur: &Value, nom: &str) -> Result<Fraction, String> {
    valeur
        .as_fraction()
        .ok_or_else(|| format!("{nom} : nombre exact attendu, reçu {valeur:?}"))
}

fn reel(valeur: &Value, nom: &str) -> Result<f64, String> {
    valeur
        .as_f64()
        .ok_or_else(|| format!("{nom} : nombre attendu, reçu {valeur:?}"))
}

/// Tous les arguments sont exacts ?
fn exacts(arguments: &[Value]) -> bool {
    arguments
        .iter()
        .all(|valeur| matches!(valeur, Value::Int(_) | Value::Rational(_)))
}

fn entier(valeur: &Value, nom: &str) -> Result<i64, String> {
    match valeur {
        Value::Int(value) => Ok(*value),
        autre => Err(format!("{nom} : entier attendu, reçu {autre:?}")),
    }
}

fn chaine(valeur: &Value, nom: &str) -> Result<String, String> {
    match valeur {
        Value::Str(texte) => Ok(texte.clone()),
        autre => Err(format!("{nom} : chaîne attendue, reçu {autre:?}")),
    }
}

fn paire(valeur: &Value, nom: &str) -> Result<(Value, Value), String> {
    match valeur {
        Value::Pair(paire) => Ok((paire.0.clone(), paire.1.clone())),
        autre => Err(format!("{nom} : paire attendue, reçu {autre:?}")),
    }
}

/// Applique une primitive nommée.
pub(crate) fn appeler(nom: &str, arguments: &[Value], depth: u32) -> Result<Value, String> {
    // Arithmétique exacte si tous les arguments le sont (R7RS §6.2.1).
    let exact = exacts(arguments);
    match nom {
        "+" => {
            if exact {
                let mut total = Fraction::ZERO;
                for argument in arguments {
                    total += nombre(argument, "+")?;
                }
                Ok(exact_value(total))
            } else {
                let mut total = 0.0;
                for argument in arguments {
                    total += reel(argument, "+")?;
                }
                Ok(Value::Real(total))
            }
        }
        "*" => {
            if exact {
                let mut total = Fraction::ONE;
                for argument in arguments {
                    total *= nombre(argument, "*")?;
                }
                Ok(exact_value(total))
            } else {
                let mut total = 1.0;
                for argument in arguments {
                    total *= reel(argument, "*")?;
                }
                Ok(Value::Real(total))
            }
        }
        "-" => match arguments {
            [] => Err("- sans argument".to_owned()),
            [seul] => {
                if exact {
                    Ok(exact_value(-nombre(seul, "-")?))
                } else {
                    Ok(Value::Real(-reel(seul, "-")?))
                }
            }
            [premier, reste @ ..] => {
                if exact {
                    let mut total = nombre(premier, "-")?;
                    for argument in reste {
                        total -= nombre(argument, "-")?;
                    }
                    Ok(exact_value(total))
                } else {
                    let mut total = reel(premier, "-")?;
                    for argument in reste {
                        total -= reel(argument, "-")?;
                    }
                    Ok(Value::Real(total))
                }
            }
        },
        "/" => match arguments {
            [] => Err("/ sans argument".to_owned()),
            [seul] => {
                if exact {
                    Ok(exact_value(Fraction::ONE / nombre(seul, "/")?))
                } else {
                    Ok(Value::Real(1.0 / reel(seul, "/")?))
                }
            }
            [premier, reste @ ..] => {
                if exact {
                    let mut total = nombre(premier, "/")?;
                    for argument in reste {
                        let diviseur = nombre(argument, "/")?;
                        if diviseur.is_zero() {
                            return Err("division par zéro".to_owned());
                        }
                        total /= diviseur;
                    }
                    Ok(exact_value(total))
                } else {
                    let mut total = reel(premier, "/")?;
                    for argument in reste {
                        total /= reel(argument, "/")?;
                    }
                    Ok(Value::Real(total))
                }
            }
        },
        "=" | "<" | ">" | "<=" | ">=" => {
            let mut precedent = match arguments.first() {
                Some(valeur) => nombre(valeur, nom)?,
                None => return Ok(Value::Bool(true)),
            };
            for argument in &arguments[1..] {
                let courant = nombre(argument, nom)?;
                let vrai = match nom {
                    "=" => precedent == courant,
                    "<" => precedent < courant,
                    ">" => precedent > courant,
                    "<=" => precedent <= courant,
                    _ => precedent >= courant,
                };
                if !vrai {
                    return Ok(Value::Bool(false));
                }
                precedent = courant;
            }
            Ok(Value::Bool(true))
        }
        "quotient" => {
            let (a, b) = deux_entiers(arguments, "quotient")?;
            if b == 0 {
                return Err("quotient : division par zéro".to_owned());
            }
            Ok(Value::Int(a / b))
        }
        "remainder" => {
            let (a, b) = deux_entiers(arguments, "remainder")?;
            if b == 0 {
                return Err("remainder : division par zéro".to_owned());
            }
            Ok(Value::Int(a % b))
        }
        "modulo" => {
            let (a, b) = deux_entiers(arguments, "modulo")?;
            if b == 0 {
                return Err("modulo : division par zéro".to_owned());
            }
            Ok(Value::Int(((a % b) + b) % b))
        }
        "abs" => {
            let valeur = arguments.first().ok_or("abs sans argument")?;
            if exact {
                Ok(exact_value(nombre(valeur, "abs")?.abs()))
            } else {
                Ok(Value::Real(reel(valeur, "abs")?.abs()))
            }
        }
        "min" | "max" => {
            let mut itere = arguments.iter();
            let premier = itere.next().ok_or("min/max sans argument")?;
            let mut meilleur = reel(premier, nom)?;
            let mut meilleur_valeur = premier.clone();
            for argument in itere {
                let valeur = reel(argument, nom)?;
                if (nom == "min" && valeur < meilleur) || (nom == "max" && valeur > meilleur) {
                    meilleur = valeur;
                    meilleur_valeur = argument.clone();
                }
            }
            Ok(meilleur_valeur)
        }
        "expt" => {
            let base = arguments.first().ok_or("expt sans base")?;
            let exposant = arguments.get(1).ok_or("expt sans exposant")?;
            if let Value::Int(puissance) = exposant {
                if *puissance >= 0 && exact {
                    let base = nombre(base, "expt")?;
                    let mut total = Fraction::ONE;
                    for _ in 0..*puissance {
                        total *= base;
                    }
                    return Ok(exact_value(total));
                }
            }
            Ok(Value::Real(
                reel(base, "expt")?.powf(reel(exposant, "expt")?),
            ))
        }
        "number?" => Ok(Value::Bool(
            arguments
                .first()
                .is_some_and(|valeur| valeur.as_f64().is_some()),
        )),
        "integer?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Int(_))
        ))),
        "rational?" | "real?" => Ok(Value::Bool(
            arguments.first().and_then(Value::as_fraction).is_some(),
        )),
        "exact?" => Ok(Value::Bool(exacts(arguments))),
        "zero?" | "positive?" | "negative?" => {
            let valeur = nombre(arguments.first().ok_or("prédicat sans argument")?, nom)?;
            Ok(Value::Bool(match nom {
                "zero?" => valeur.is_zero(),
                "positive?" => valeur.is_positive(),
                _ => !valeur.is_positive() && !valeur.is_zero(),
            }))
        }
        "number->string" => {
            let valeur = arguments.first().ok_or("number->string sans argument")?;
            Ok(Value::Str(match valeur {
                Value::Int(entier) => entier.to_string(),
                Value::Rational(fraction) => {
                    if fraction.denominator() == 1 {
                        fraction.numerator().to_string()
                    } else {
                        format!("{}/{}", fraction.numerator(), fraction.denominator())
                    }
                }
                Value::Real(reel) => format!("{reel}"),
                autre => return Err(format!("number->string : {autre:?}")),
            }))
        }
        "string->number" => {
            let texte = chaine(
                arguments.first().ok_or("string->number sans argument")?,
                nom,
            )?;
            Ok(super::datum::nombre_depuis_texte(&texte).unwrap_or(Value::Bool(false)))
        }
        "not" => Ok(Value::Bool(!arguments.first().is_some_and(Value::truthy))),
        "boolean?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Bool(_))
        ))),
        "cons" => {
            let tete = arguments.first().cloned().ok_or("cons sans tête")?;
            let queue = arguments.get(1).cloned().ok_or("cons sans queue")?;
            Ok(Value::Pair(std::rc::Rc::new((tete, queue))))
        }
        "car" | "cdr" => {
            let (tete, queue) = paire(arguments.first().ok_or(nom)?, nom)?;
            Ok(if nom == "car" { tete } else { queue })
        }
        "caar" | "cadr" | "cdar" | "cddr" => {
            let valeur = arguments.first().ok_or(nom)?.clone();
            let (premier, deuxieme) = (nom.chars().nth(1), nom.chars().nth(2));
            let etape = |valeur: Value, cle: Option<char>| -> Result<Value, String> {
                let (tete, queue) = paire(&valeur, nom)?;
                Ok(if cle == Some('d') { queue } else { tete })
            };
            let intermediaire = etape(valeur, deuxieme)?;
            etape(intermediaire, premier)
        }
        "list" => Ok(liste(arguments.to_vec())),
        "null?" => Ok(Value::Bool(matches!(arguments.first(), Some(Value::Nil)))),
        "pair?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Pair(_))
        ))),
        "list?" => {
            Ok(Value::Bool(arguments.first().is_some_and(|valeur| {
                valeur.clone().into_list().is_some()
            })))
        }
        "length" => {
            let elements = arguments
                .first()
                .cloned()
                .ok_or("length sans argument")?
                .into_list()
                .ok_or("length : liste attendue")?;
            Ok(Value::Int(elements.len() as i64))
        }
        "append" => {
            let mut resultat = arguments.last().cloned().unwrap_or(Value::Nil);
            for argument in arguments[..arguments.len().saturating_sub(1)].iter().rev() {
                let elements = argument
                    .clone()
                    .into_list()
                    .ok_or("append : liste attendue")?;
                for element in elements.into_iter().rev() {
                    resultat = Value::Pair(std::rc::Rc::new((element, resultat)));
                }
            }
            Ok(resultat)
        }
        "reverse" => {
            let mut elements = arguments
                .first()
                .cloned()
                .ok_or("reverse sans argument")?
                .into_list()
                .ok_or("reverse : liste attendue")?;
            elements.reverse();
            Ok(liste(elements))
        }
        "list-ref" => {
            let elements = arguments
                .first()
                .cloned()
                .ok_or("list-ref sans liste")?
                .into_list()
                .ok_or("list-ref : liste attendue")?;
            let index = entier(arguments.get(1).ok_or("list-ref sans index")?, nom)?;
            elements
                .get(usize::try_from(index).unwrap_or(usize::MAX))
                .cloned()
                .ok_or_else(|| "list-ref : indice hors liste".to_owned())
        }
        "list-tail" => {
            let elements = arguments
                .first()
                .cloned()
                .ok_or("list-tail sans liste")?
                .into_list()
                .ok_or("list-tail : liste attendue")?;
            let index = entier(arguments.get(1).ok_or("list-tail sans index")?, nom)?;
            Ok(liste(
                elements
                    .get(usize::try_from(index).unwrap_or(usize::MAX)..)
                    .unwrap_or_default()
                    .to_vec(),
            ))
        }
        "member" | "memq" => {
            let cible = arguments.first().ok_or("member sans cible")?;
            let elements = arguments
                .get(1)
                .cloned()
                .ok_or("member sans liste")?
                .into_list()
                .ok_or("member : liste attendue")?;
            for (index, element) in elements.iter().enumerate() {
                if element == cible || (nom == "memq" && eqv(element, cible)) {
                    return Ok(liste(elements[index..].to_vec()));
                }
            }
            Ok(Value::Bool(false))
        }
        "assoc" | "assq" => {
            let cible = arguments.first().ok_or("assoc sans cible")?;
            let elements = arguments
                .get(1)
                .cloned()
                .ok_or("assoc sans liste")?
                .into_list()
                .ok_or("assoc : liste attendue")?;
            for element in elements {
                let Ok((cle, _)) = paire(&element, nom) else {
                    continue;
                };
                if cle == *cible || (nom == "assq" && eqv(&cle, cible)) {
                    return Ok(element);
                }
            }
            Ok(Value::Bool(false))
        }
        "map" | "for-each" => {
            let fonction = arguments.first().cloned().ok_or("map sans fonction")?;
            let mut listes = Vec::new();
            for argument in &arguments[1..] {
                listes.push(argument.clone().into_list().ok_or("map : liste attendue")?);
            }
            let longueur = listes.iter().map(Vec::len).min().unwrap_or(0);
            let mut resultats = Vec::with_capacity(longueur);
            for index in 0..longueur {
                let valeurs: Vec<Value> = listes
                    .iter()
                    .map(|elements| elements[index].clone())
                    .collect();
                resultats.push(apply(&fonction, &valeurs, depth + 1)?);
            }
            if nom == "for-each" {
                Ok(Value::Unspecified)
            } else {
                Ok(liste(resultats))
            }
        }
        "every" | "any" => {
            let fonction = arguments.first().cloned().ok_or("every sans fonction")?;
            let elements = arguments
                .get(1)
                .cloned()
                .ok_or("every sans liste")?
                .into_list()
                .ok_or("every : liste attendue")?;
            for element in elements {
                let resultat = apply(&fonction, &[element], depth + 1)?;
                let vrai = resultat.truthy();
                if (nom == "every" && !vrai) || (nom == "any" && vrai) {
                    return Ok(resultat);
                }
            }
            Ok(if nom == "every" {
                Value::Bool(true)
            } else {
                Value::Bool(false)
            })
        }
        "apply" => {
            let fonction = arguments.first().cloned().ok_or("apply sans fonction")?;
            let mut valeurs = arguments[1..arguments.len().saturating_sub(1)].to_vec();
            if let Some(derniere) = arguments.last() {
                valeurs.extend(
                    derniere
                        .clone()
                        .into_list()
                        .ok_or("apply : dernière liste attendue")?,
                );
            }
            apply(&fonction, &valeurs, depth + 1)
        }
        "iota" => {
            let compte = entier(arguments.first().ok_or("iota sans compte")?, nom)?;
            let depart = arguments
                .get(1)
                .map(|valeur| entier(valeur, nom))
                .transpose()?
                .unwrap_or(0);
            let pas = arguments
                .get(2)
                .map(|valeur| entier(valeur, nom))
                .transpose()?
                .unwrap_or(1);
            Ok(liste(
                (0..compte.max(0))
                    .map(|index| Value::Int(depart + index * pas))
                    .collect(),
            ))
        }
        "symbol?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Symbol(_))
        ))),
        "symbol->string" => match arguments.first() {
            Some(Value::Symbol(symbole)) => Ok(Value::Str(symbole.clone())),
            Some(Value::Str(texte)) => Ok(Value::Str(texte.clone())),
            autre => Err(format!("symbol->string : {autre:?}")),
        },
        "string->symbol" => Ok(Value::Symbol(chaine(
            arguments.first().ok_or("string->symbol sans argument")?,
            nom,
        )?)),
        "string?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Str(_))
        ))),
        "string-append" => {
            let mut texte = String::new();
            for argument in arguments {
                texte.push_str(&chaine(argument, nom)?);
            }
            Ok(Value::Str(texte))
        }
        "string-length" => Ok(Value::Int(
            chaine(arguments.first().ok_or("string-length")?, nom)?
                .chars()
                .count() as i64,
        )),
        "string=?" => {
            let mut itere = arguments.iter();
            let Some(premier) = itere.next() else {
                return Ok(Value::Bool(true));
            };
            let premier = chaine(premier, nom)?;
            Ok(Value::Bool(itere.all(|argument| {
                chaine(argument, nom).is_ok_and(|texte| texte == premier)
            })))
        }
        "char?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Char(_))
        ))),
        "char->integer" => match arguments.first() {
            Some(Value::Char(character)) => Ok(Value::Int(i64::from(*character as u32))),
            autre => Err(format!("char->integer : {autre:?}")),
        },
        "integer->char" => {
            let code = entier(arguments.first().ok_or("integer->char")?, nom)?;
            Ok(Value::Char(
                char::from_u32(u32::try_from(code).unwrap_or(32)).unwrap_or(' '),
            ))
        }
        "vector?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Vector(_))
        ))),
        "vector" => Ok(Value::Vector(std::rc::Rc::new(arguments.to_vec()))),
        "vector-length" => match arguments.first() {
            Some(Value::Vector(elements)) => Ok(Value::Int(elements.len() as i64)),
            autre => Err(format!("vector-length : {autre:?}")),
        },
        "vector-ref" => match arguments.first() {
            Some(Value::Vector(elements)) => {
                let index = entier(arguments.get(1).ok_or("vector-ref sans index")?, nom)?;
                elements
                    .get(usize::try_from(index).unwrap_or(usize::MAX))
                    .cloned()
                    .ok_or_else(|| "vector-ref : indice hors vecteur".to_owned())
            }
            autre => Err(format!("vector-ref : {autre:?}")),
        },
        "vector->list" => match arguments.first() {
            Some(Value::Vector(elements)) => Ok(liste(elements.as_ref().clone())),
            autre => Err(format!("vector->list : {autre:?}")),
        },
        "list->vector" => {
            let elements = arguments
                .first()
                .cloned()
                .ok_or("list->vector sans argument")?
                .into_list()
                .ok_or("list->vector : liste attendue")?;
            Ok(Value::Vector(std::rc::Rc::new(elements)))
        }
        "procedure?" => Ok(Value::Bool(matches!(
            arguments.first(),
            Some(Value::Proc(_)) | Some(Value::Builtin(_))
        ))),
        "eq?" | "eqv?" => Ok(Value::Bool(
            arguments.len() == 2 && eqv(&arguments[0], &arguments[1]),
        )),
        "equal?" => Ok(Value::Bool(
            arguments.len() == 2 && arguments[0] == arguments[1],
        )),
        "error" => Err(match arguments.first() {
            Some(Value::Str(texte)) => texte.clone(),
            Some(autre) => format!("erreur Scheme : {autre:?}"),
            None => "erreur Scheme".to_owned(),
        }),
        "display" | "newline" => Ok(Value::Unspecified),
        "format" => Ok(format_scheme(arguments)),
        // LilyPond : valeurs.
        "ly:make-moment" => match arguments {
            [seul] => Ok(Value::Rational(nombre(seul, nom)?)),
            [numerateur, denominateur] => {
                let numerateur = entier(numerateur, nom)?;
                let denominateur = entier(denominateur, nom)?;
                Ok(exact_value(
                    Fraction::try_new(numerateur, denominateur)
                        .ok_or("ly:make-moment : dénominateur nul")?,
                ))
            }
            _ => Err("ly:make-moment : arguments invalides".to_owned()),
        },
        "ly:make-duration" => {
            // `(ly:make-duration log2 [dots [num [den]]])`
            // (`lily/duration-scheme.cc:53-99`) : la durée vaut
            // (num/den) × 1/2^log2, allongée par `dots`.
            let log2 =
                entier(arguments.first().ok_or("ly:make-duration")?, nom)?.clamp(0, 12) as u32;
            let points = arguments
                .get(1)
                .map(|valeur| entier(valeur, nom))
                .transpose()?
                .unwrap_or(0)
                .clamp(0, 4) as u32;
            let numerateur = arguments
                .get(2)
                .map(|valeur| entier(valeur, nom))
                .transpose()?
                .unwrap_or(1);
            let denominateur = arguments
                .get(3)
                .map(|valeur| entier(valeur, nom))
                .transpose()?
                .unwrap_or(1);
            let base = Fraction::try_new(numerateur, denominateur)
                .ok_or("ly:make-duration : dénominateur nul")?
                / Fraction::from_integer(1_i64 << log2);
            let facteur = Fraction::from_integer((2_i64 << points) - 1)
                / Fraction::from_integer(1_i64 << points);
            Ok(exact_value(base * facteur))
        }
        "ly:make-pitch" | "ly:pitch?" | "ly:duration?" | "ly:music?" => Ok(Value::Unspecified),
        "magstep" => {
            let pas = arguments
                .first()
                .map_or(0.0, |valeur| valeur.as_f64().unwrap_or(0.0));
            Ok(Value::Real(2f64.powf(pas / 1200.0)))
        }
        // Configuration : consommée, sans effet sur le modèle.
        "ly:set-option"
        | "ly:expect-warning"
        | "set-global-staff-size"
        | "set-paper-size"
        | "ly:parser-set-note-names"
        | "ly:parser-include-string"
        | "ly:message"
        | "ly:warning"
        | "ly:progress"
        | "ly:debug"
        | "ly:gulp-file" => Ok(Value::Unspecified),
        "define" => Err("define hors d'une forme".to_owned()),
        autre => Err(format!("primitive Scheme inconnue : {autre}")),
    }
}

fn deux_entiers(arguments: &[Value], nom: &str) -> Result<(i64, i64), String> {
    let a = entier(arguments.first().ok_or(nom)?, nom)?;
    let b = entier(arguments.get(1).ok_or(nom)?, nom)?;
    Ok((a, b))
}

/// `eqv?` (R7RS §6.1) : identité pour les nombres et caractères, égalité pour
/// les symboles.
fn eqv(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Rational(x), Value::Rational(y)) => x == y,
        (Value::Real(x), Value::Real(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        (Value::Symbol(x), Value::Symbol(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Nil, Value::Nil) => true,
        (Value::Builtin(x), Value::Builtin(y)) => x == y,
        (Value::Proc(x), Value::Proc(y)) => std::rc::Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// `format` minimal (`~a`, `~s`, `~%`) pour les textes de portée.
fn format_scheme(arguments: &[Value]) -> Value {
    let mut texte = String::new();
    let mut iterateur = arguments.iter();
    let gabarit = match arguments.first() {
        Some(Value::Bool(false)) => arguments.get(1),
        _ => arguments.first(),
    };
    let Some(Value::Str(gabarit)) = gabarit else {
        return Value::Str(String::new());
    };
    let mut valeurs = arguments
        .iter()
        .skip_while(|valeur| !matches!(valeur, Value::Str(texte) if texte == gabarit));
    valeurs.next();
    let _ = iterateur.next();
    let mut caracteres = gabarit.chars().peekable();
    while let Some(character) = caracteres.next() {
        if character != '~' {
            texte.push(character);
            continue;
        }
        match caracteres.next() {
            Some('a') | Some('s') => {
                if let Some(valeur) = valeurs.next() {
                    texte.push_str(&affiche(valeur));
                }
            }
            Some('%') => texte.push('\n'),
            Some('~') => texte.push('~'),
            Some(autre) => {
                texte.push('~');
                texte.push(autre);
            }
            None => texte.push('~'),
        }
    }
    Value::Str(texte)
}

fn affiche(valeur: &Value) -> String {
    match valeur {
        Value::Str(texte) => texte.clone(),
        Value::Int(entier) => entier.to_string(),
        Value::Rational(fraction) => {
            if fraction.denominator() == 1 {
                fraction.numerator().to_string()
            } else {
                format!("{}/{}", fraction.numerator(), fraction.denominator())
            }
        }
        Value::Real(reel) => format!("{reel}"),
        Value::Symbol(symbole) => symbole.clone(),
        Value::Bool(true) => "#t".to_owned(),
        Value::Bool(false) => "#f".to_owned(),
        Value::Char(character) => character.to_string(),
        Value::Nil => "()".to_owned(),
        autre => format!("{autre:?}"),
    }
}
