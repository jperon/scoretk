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

//! Évaluateur : environnements lexicaux, formes spéciales et primitives.
//!
//! Sémantique R7RS-small : §4.1.3 (appels), §4.1.4 (`lambda`), §4.1.5
//! (`if`), §4.1.6 (`set!`), §4.2.1 (`cond`, `case`, `and`, `or`, `when`,
//! `unless`), §4.2.2 (`let`, `let*`, `letrec`), §4.2.8 (quasiquotation),
//! §5.3 (`define`), §6.2–6.8 (nombres, booléens, paires, symboles, chaînes,
//! vecteurs). Les primitives LilyPond suivent `lily/duration-scheme.cc`,
//! `lily/moment-scheme.cc` et `scm/lily.scm` ; les fonctions d'effet de bord
//! (`ly:set-option`, `ly:expect-warning`, `set-global-staff-size`…) rendent
//! `#<unspecified>` : le lecteur n'en a besoin que comme datum consommé
//! (`lily/parser.yy:565-615`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::datum::{liste, Value};

/// Environnement lexical (R7RS §4.1.1 : une trame par portée).
pub(crate) type Env = Rc<RefCell<Scope>>;

/// Trame de variables, chaînée vers la portée englobante.
#[derive(Debug)]
pub(crate) struct Scope {
    variables: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Scope {
    fn new(parent: Option<Env>) -> Env {
        Rc::new(RefCell::new(Self {
            variables: HashMap::new(),
            parent,
        }))
    }
}

/// Procédure `lambda`.
#[derive(Clone)]
pub(crate) struct Proc {
    pub(crate) params: Vec<String>,
    pub(crate) rest: Option<String>,
    pub(crate) body: Vec<Value>,
    pub(crate) env: Env,
}

impl std::fmt::Debug for Proc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proc")
            .field("params", &self.params)
            .finish()
    }
}

/// Profondeur maximale d'évaluation (garde-fou contre les boucles).
const MAX_DEPTH: u32 = 256;

/// Portée enfant de `parent` (corps de fonction).
pub(crate) fn child_env(parent: &Env) -> Env {
    Scope::new(Some(parent.clone()))
}

/// Lie une variable dans une portée.
pub(crate) fn define_value(env: &mut Env, nom: String, valeur: Value) {
    define(env, nom, valeur);
}

fn lookup(env: &Env, nom: &str) -> Option<Value> {
    let mut courant = Some(env.clone());
    while let Some(trame) = courant {
        let emprunt = trame.borrow();
        if let Some(valeur) = emprunt.variables.get(nom) {
            return Some(valeur.clone());
        }
        courant = emprunt.parent.clone();
    }
    None
}

fn define(env: &Env, nom: String, valeur: Value) {
    env.borrow_mut().variables.insert(nom, valeur);
}

fn set(env: &Env, nom: &str, valeur: Value) -> Result<(), String> {
    let mut courant = Some(env.clone());
    while let Some(trame) = courant {
        let mut emprunt = trame.borrow_mut();
        if emprunt.variables.contains_key(nom) {
            emprunt.variables.insert(nom.to_owned(), valeur);
            return Ok(());
        }
        courant = emprunt.parent.clone();
    }
    Err(format!("set! sur une variable non liée : {nom}"))
}

/// Environnement global : primitives Scheme et LilyPond.
pub(crate) fn global_env() -> Env {
    let env = Scope::new(None);
    // Constantes de Guile/LilyPond utiles aux fichiers réels.
    for (nom, valeur) in [
        ("#t", Value::Bool(true)),
        ("#f", Value::Bool(false)),
        ("t", Value::Symbol("t".to_owned())),
        ("nil", Value::Nil),
    ] {
        define(&env, nom.to_owned(), valeur);
    }
    for primitive in PRIMITIVES {
        define(&env, (*primitive).to_owned(), Value::Builtin(primitive));
    }
    env
}

// Catalogue des primitives reconnues. Les noms LilyPond `ly:*` sans
// signification pour le modèle rendent `#<unspecified>`.
const PRIMITIVES: &[&str] = &[
    // Arithmétique et comparaisons (R7RS §6.2).
    "+",
    "-",
    "*",
    "/",
    "=",
    "<",
    ">",
    "<=",
    ">=",
    "quotient",
    "remainder",
    "modulo",
    "abs",
    "min",
    "max",
    "expt",
    "number?",
    "integer?",
    "rational?",
    "real?",
    "exact?",
    "zero?",
    "positive?",
    "negative?",
    "number->string",
    "string->number",
    // Booléens.
    "not",
    "boolean?",
    // Paires et listes (R7RS §6.4).
    "cons",
    "car",
    "cdr",
    "caar",
    "cadr",
    "cdar",
    "cddr",
    "list",
    "null?",
    "pair?",
    "list?",
    "length",
    "append",
    "reverse",
    "list-ref",
    "list-tail",
    "member",
    "memq",
    "assoc",
    "assq",
    "map",
    "for-each",
    "apply",
    "iota",
    "list?",
    "every",
    "any",
    // Symboles, chaînes, caractères.
    "symbol?",
    "symbol->string",
    "string->symbol",
    "string?",
    "string-append",
    "string-length",
    "string=?",
    "char?",
    "char->integer",
    "integer->char",
    // Vecteurs.
    "vector?",
    "vector",
    "vector-length",
    "vector-ref",
    "vector->list",
    "list->vector",
    // Divers.
    "procedure?",
    "eq?",
    "eqv?",
    "equal?",
    "error",
    "display",
    "newline",
    "format",
    // LilyPond : valeurs numériques et musicales.
    "ly:make-moment",
    "ly:make-duration",
    "ly:make-pitch",
    "ly:pitch?",
    "ly:duration?",
    "ly:music?",
    "magstep",
    "ly:set-option",
    "ly:expect-warning",
    "set-global-staff-size",
    "set-paper-size",
    "ly:parser-set-note-names",
    "ly:parser-include-string",
    "define",
];

/// Évalue une valeur (datum) dans un environnement.
pub(crate) fn eval(valeur: &Value, env: &mut Env, depth: u32) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("profondeur d'évaluation Scheme dépassée".to_owned());
    }
    match valeur {
        Value::Symbol(nom) => {
            // `#t` de LilyPond est le symbole `t` : le lier à lui-même.
            lookup(env, nom).ok_or_else(|| format!("symbole Scheme non lié : {nom}"))
        }
        Value::Pair(_) => eval_pair(valeur, env, depth),
        Value::Proc(_) | Value::Builtin(_) => Ok(valeur.clone()),
        autre => Ok(autre.clone()),
    }
}

fn eval_pair(valeur: &Value, env: &mut Env, depth: u32) -> Result<Value, String> {
    let liste = valeur
        .clone()
        .into_list()
        .ok_or("appel Scheme avec une paire pointue")?;
    let Some(tete) = liste.first() else {
        return Err("appel Scheme vide".to_owned());
    };
    let arguments = &liste[1..];
    let nom = match tete {
        Value::Symbol(nom) => nom.as_str(),
        _ => "",
    };
    match nom {
        "quote" => arguments
            .first()
            .cloned()
            .ok_or("quote sans argument".to_owned()),
        "quasiquote" => {
            let forme = arguments.first().ok_or("quasiquote sans argument")?;
            quasiquote(forme, env, depth)
        }
        "if" => {
            let (Some(test), Some(alors)) = (arguments.first(), arguments.get(1)) else {
                return Err("if mal formé".to_owned());
            };
            let condition = eval(test, env, depth + 1)?;
            if condition.truthy() {
                eval(alors, env, depth + 1)
            } else {
                match arguments.get(2) {
                    Some(sinon) => eval(sinon, env, depth + 1),
                    None => Ok(Value::Unspecified),
                }
            }
        }
        "cond" => {
            for clause in arguments {
                let elements = clause.clone().into_list().ok_or("clause cond mal formée")?;
                let Some(test) = elements.first() else {
                    continue;
                };
                if matches!(test, Value::Symbol(symbole) if symbole == "else") {
                    return eval_body(&elements[1..], env, depth + 1);
                }
                if eval(test, env, depth + 1)?.truthy() {
                    if elements.len() == 1 {
                        return eval(test, env, depth + 1);
                    }
                    return eval_body(&elements[1..], env, depth + 1);
                }
            }
            Ok(Value::Unspecified)
        }
        "case" => {
            let Some(cle) = arguments.first() else {
                return Err("case mal formé".to_owned());
            };
            let cle = eval(cle, env, depth + 1)?;
            for clause in &arguments[1..] {
                let elements = clause.clone().into_list().ok_or("clause case mal formée")?;
                let Some(donnees) = elements.first() else {
                    continue;
                };
                let correspond = match donnees {
                    Value::Symbol(symbole) if symbole == "else" => true,
                    autre => autre
                        .clone()
                        .into_list()
                        .is_some_and(|valeurs| valeurs.contains(&cle)),
                };
                if correspond {
                    return eval_body(&elements[1..], env, depth + 1);
                }
            }
            Ok(Value::Unspecified)
        }
        "and" => {
            let mut resultat = Value::Bool(true);
            for argument in arguments {
                resultat = eval(argument, env, depth + 1)?;
                if !resultat.truthy() {
                    return Ok(resultat);
                }
            }
            Ok(resultat)
        }
        "or" => {
            for argument in arguments {
                let resultat = eval(argument, env, depth + 1)?;
                if resultat.truthy() {
                    return Ok(resultat);
                }
            }
            Ok(Value::Bool(false))
        }
        "when" | "unless" => {
            let (Some(test), Some(corps)) = (arguments.first(), arguments.get(1)) else {
                return Err("when/unless mal formé".to_owned());
            };
            let condition = eval(test, env, depth + 1)?.truthy();
            if condition == (nom == "when") {
                eval(corps, env, depth + 1)
            } else {
                Ok(Value::Unspecified)
            }
        }
        "begin" => eval_body(arguments, env, depth + 1),
        "define" => {
            let Some(cible) = arguments.first() else {
                return Err("define sans cible".to_owned());
            };
            match cible {
                Value::Symbol(symbole) => {
                    let valeur = match arguments.get(1) {
                        Some(forme) => eval(forme, env, depth + 1)?,
                        None => Value::Unspecified,
                    };
                    define(env, symbole.clone(), valeur);
                    Ok(Value::Unspecified)
                }
                Value::Pair(_) => {
                    // Forme courte `(define (f x) …)` (R7RS §5.3).
                    let elements = cible.clone().into_list().ok_or("define mal formé")?;
                    let Some(Value::Symbol(symbole)) = elements.first() else {
                        return Err("define mal formé".to_owned());
                    };
                    let procede = Value::Proc(Rc::new(Proc {
                        params: elements[1..]
                            .iter()
                            .map(|element| match element {
                                Value::Symbol(nom) => nom.clone(),
                                _ => String::new(),
                            })
                            .collect(),
                        rest: None,
                        body: arguments[1..].to_vec(),
                        env: env.clone(),
                    }));
                    define(env, symbole.clone(), procede);
                    Ok(Value::Unspecified)
                }
                _ => Err("define mal formé".to_owned()),
            }
        }
        "set!" => {
            let (Some(Value::Symbol(nom)), Some(forme)) = (arguments.first(), arguments.get(1))
            else {
                return Err("set! mal formé".to_owned());
            };
            let valeur = eval(forme, env, depth + 1)?;
            set(env, nom, valeur)?;
            Ok(Value::Unspecified)
        }
        "lambda" => {
            let Some(parameters) = arguments.first() else {
                return Err("lambda sans paramètres".to_owned());
            };
            let (params, rest) = parametres(parameters)?;
            Ok(Value::Proc(Rc::new(Proc {
                params,
                rest,
                body: arguments[1..].to_vec(),
                env: env.clone(),
            })))
        }
        "let" => {
            let (Some(liaisons), Some(_)) = (arguments.first(), arguments.get(1)) else {
                return Err("let mal formé".to_owned());
            };
            let mut portee = Scope::new(Some(env.clone()));
            for liaison in liaisons
                .clone()
                .into_list()
                .ok_or("liaisons let mal formées")?
            {
                let elements = liaison.into_list().ok_or("liaison let mal formée")?;
                let (Some(Value::Symbol(nom)), Some(forme)) = (elements.first(), elements.get(1))
                else {
                    return Err("liaison let mal formée".to_owned());
                };
                let valeur = eval(forme, env, depth + 1)?;
                define(&portee, nom.clone(), valeur);
            }
            eval_body(&arguments[1..], &mut portee, depth + 1)
        }
        "let*" => {
            let (Some(liaisons), Some(_)) = (arguments.first(), arguments.get(1)) else {
                return Err("let* mal formé".to_owned());
            };
            let mut portee = env.clone();
            for liaison in liaisons
                .clone()
                .into_list()
                .ok_or("liaisons let* mal formées")?
            {
                let elements = liaison.into_list().ok_or("liaison let* mal formée")?;
                let (Some(Value::Symbol(nom)), Some(forme)) = (elements.first(), elements.get(1))
                else {
                    return Err("liaison let* mal formée".to_owned());
                };
                let suivante = Scope::new(Some(portee.clone()));
                let valeur = eval(forme, &mut portee.clone(), depth + 1)?;
                define(&suivante, nom.clone(), valeur);
                portee = suivante;
            }
            eval_body(&arguments[1..], &mut portee, depth + 1)
        }
        "letrec" => {
            let (Some(liaisons), Some(_)) = (arguments.first(), arguments.get(1)) else {
                return Err("letrec mal formé".to_owned());
            };
            let liaisons = liaisons
                .clone()
                .into_list()
                .ok_or("liaisons letrec mal formées")?;
            let portee = Scope::new(Some(env.clone()));
            let mut noms = Vec::new();
            for liaison in &liaisons {
                let elements = liaison
                    .clone()
                    .into_list()
                    .ok_or("liaison letrec mal formée")?;
                let Some(Value::Symbol(nom)) = elements.first() else {
                    return Err("liaison letrec mal formée".to_owned());
                };
                noms.push(nom.clone());
                define(&portee, nom.clone(), Value::Unspecified);
            }
            for (liaison, nom) in liaisons.iter().zip(noms) {
                let elements = liaison
                    .clone()
                    .into_list()
                    .ok_or("liaison letrec mal formée")?;
                let Some(forme) = elements.get(1) else {
                    continue;
                };
                let mut contexte = portee.clone();
                let valeur = eval(forme, &mut contexte, depth + 1)?;
                define(&portee, nom, valeur);
            }
            eval_body(&arguments[1..], &mut portee.clone(), depth + 1)
        }
        _ => {
            // Application de procédure (R7RS §4.1.3).
            let operateur = eval(tete, env, depth + 1)?;
            let mut valeurs = Vec::with_capacity(arguments.len());
            for argument in arguments {
                valeurs.push(eval(argument, env, depth + 1)?);
            }
            apply(&operateur, &valeurs, depth + 1)
        }
    }
}

fn parametres(valeur: &Value) -> Result<(Vec<String>, Option<String>), String> {
    match valeur {
        Value::Symbol(nom) => Ok((Vec::new(), Some(nom.clone()))),
        Value::Pair(_) => {
            // Liste, éventuellement pointue : `(a b . reste)`.
            let mut params = Vec::new();
            let mut courant = valeur.clone();
            let mut rest = None;
            loop {
                match courant {
                    Value::Nil => break,
                    Value::Pair(paire) => {
                        match &paire.0 {
                            Value::Symbol(nom) => params.push(nom.clone()),
                            _ => return Err("paramètre non symbolique".to_owned()),
                        }
                        courant = paire.1.clone();
                    }
                    Value::Symbol(nom) => {
                        rest = Some(nom);
                        break;
                    }
                    _ => return Err("liste de paramètres mal formée".to_owned()),
                }
            }
            Ok((params, rest))
        }
        _ => Err("paramètres lambda mal formés".to_owned()),
    }
}

fn eval_body(corps: &[Value], env: &mut Env, depth: u32) -> Result<Value, String> {
    let mut resultat = Value::Unspecified;
    for forme in corps {
        resultat = eval(forme, env, depth)?;
    }
    Ok(resultat)
}

/// Applique une procédure à des arguments déjà évalués.
pub(crate) fn apply(operateur: &Value, arguments: &[Value], depth: u32) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("profondeur d'évaluation Scheme dépassée".to_owned());
    }
    match operateur {
        Value::Builtin(nom) => super::primitives::appeler(nom, arguments, depth),
        Value::Proc(procédure) => {
            if arguments.len() < procédure.params.len()
                || (procédure.rest.is_none() && arguments.len() > procédure.params.len())
            {
                return Err(format!(
                    "arité : {} attendus, {} reçus",
                    procédure.params.len(),
                    arguments.len()
                ));
            }
            let mut portee = Scope::new(Some(procédure.env.clone()));
            for (nom, valeur) in procédure.params.iter().zip(arguments.iter()) {
                define(&portee, nom.clone(), valeur.clone());
            }
            if let Some(reste) = &procédure.rest {
                define(
                    &portee,
                    reste.clone(),
                    liste(arguments[procédure.params.len()..].to_vec()),
                );
            }
            eval_body(&procédure.body, &mut portee, depth + 1)
        }
        _ => Err("application d'une valeur qui n'est pas une procédure".to_owned()),
    }
}

/// Quasiquotation (R7RS §4.2.8).
fn quasiquote(forme: &Value, env: &mut Env, depth: u32) -> Result<Value, String> {
    match forme {
        Value::Pair(_) => {
            let elements = forme
                .clone()
                .into_list()
                .ok_or("quasiquote d'une paire pointue")?;
            if let Some(Value::Symbol(symbole)) = elements.first() {
                if symbole == "unquote" {
                    return eval(
                        elements.get(1).ok_or("unquote sans argument")?,
                        env,
                        depth + 1,
                    );
                }
            }
            let mut resultat = Vec::new();
            for element in elements {
                if let Value::Pair(_) = &element {
                    if let Some(sous) = element.clone().into_list() {
                        if matches!(sous.first(), Some(Value::Symbol(symbole)) if symbole == "unquote-splicing")
                        {
                            let morceau = eval(
                                sous.get(1).ok_or("unquote-splicing sans argument")?,
                                env,
                                depth + 1,
                            )?;
                            resultat.extend(morceau.into_list().unwrap_or_default());
                            continue;
                        }
                    }
                }
                resultat.push(quasiquote(&element, env, depth + 1)?);
            }
            Ok(liste(resultat))
        }
        _ => Ok(forme.clone()),
    }
}
