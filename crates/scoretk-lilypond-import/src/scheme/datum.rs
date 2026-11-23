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

//! Lecteur de datums Scheme (R7RS-small §7.1.2 « External representations »).
//!
//! Le lexer LilyPond fournit déjà le texte d'**un** datum (`#`/`$` followed by
//! Guile's `scm_read`, `lily/lexer.ll:407-441`). Ce module le relit en valeurs
//! typées : booléens, entiers, rationnels exacts, réels, chaînes, caractères,
//! symboles, listes et paires pointues, vecteurs, citations et musique
//! embarquée `#{ … #}` (LilyPond, pas Scheme : `lily/lexer.ll:421-441`).

use std::rc::Rc;

use scoretk_core::model::Fraction;

use super::eval::Proc;

/// Valeur Scheme.
#[derive(Clone, Debug)]
pub(crate) enum Value {
    /// `#t` / `#f` (R7RS §6.3).
    Bool(bool),
    /// Entier exact.
    Int(i64),
    /// Rationnel exact (`#3/4`, résultat de `/`).
    Rational(Fraction),
    /// Réel inexact.
    Real(f64),
    /// Chaîne (R7RS §6.7).
    Str(String),
    /// Caractère `#\a` (R7RS §6.6).
    Char(char),
    /// Symbole (R7RS §6.5), y compris `t` (`#t` de LilyPond est le symbole
    /// `t` : `#` ne lit qu'un datum, `##t` est le booléen).
    Symbol(String),
    /// Liste vide.
    Nil,
    /// Paire (R7RS §6.4).
    Pair(Rc<(Value, Value)>),
    /// Vecteur `#(…)` (R7RS §6.8).
    Vector(Rc<Vec<Value>>),
    /// Procédure `lambda`.
    Proc(Rc<Proc>),
    /// Primitive du catalogue ([`super::eval`]).
    Builtin(&'static str),
    /// Musique embarquée `#{ … #}`, transmise telle quelle au parseur.
    Embedded(String),
    /// Argument musical d'une fonction musicale (valeur opaque pour
    /// l'évaluateur, rendue au parseur si le corps la renvoie).
    Music(usize),
    /// Résultat sans intérêt (`#<unspecified>`, `define`).
    Unspecified,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Rational(a), Self::Rational(b)) => a == b,
            (Self::Real(a), Self::Real(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Char(a), Self::Char(b)) => a == b,
            (Self::Symbol(a), Self::Symbol(b)) => a == b,
            (Self::Nil, Self::Nil) => true,
            (Self::Pair(a), Self::Pair(b)) => a == b,
            (Self::Vector(a), Self::Vector(b)) => a == b,
            (Self::Proc(a), Self::Proc(b)) => Rc::ptr_eq(a, b),
            (Self::Builtin(a), Self::Builtin(b)) => a == b,
            (Self::Embedded(a), Self::Embedded(b)) => a == b,
            (Self::Music(a), Self::Music(b)) => a == b,
            (Self::Unspecified, Self::Unspecified) => true,
            _ => false,
        }
    }
}

impl Value {
    /// Vrai si la valeur est vraie au sens de Scheme : seul `#f` est faux
    /// (R7RS §6.3).
    pub(crate) fn truthy(&self) -> bool {
        !matches!(self, Self::Bool(false))
    }

    /// Liste propre (R7RS §6.4), ou `None` pour une paire pointue.
    pub(crate) fn into_list(self) -> Option<Vec<Self>> {
        let mut elements = Vec::new();
        let mut courant = self;
        loop {
            match courant {
                Self::Nil => return Some(elements),
                Self::Pair(pair) => {
                    let (tete, queue) = Rc::try_unwrap(pair)
                        .unwrap_or_else(|pair| (pair.0.clone(), pair.1.clone()));
                    elements.push(tete);
                    courant = queue;
                }
                _ => return None,
            }
        }
    }

    /// Représentation d'un nombre exact si possible.
    pub(crate) fn as_fraction(&self) -> Option<Fraction> {
        match self {
            Self::Int(value) => Some(Fraction::from_integer(*value)),
            Self::Rational(value) => Some(*value),
            Self::Real(value) if value.fract() == 0.0 && value.is_finite() => {
                Some(Fraction::from_integer(*value as i64))
            }
            // Paire pointue `(n . d)` : `\scaleDurations #'(2 . 3)`
            // (`lily/parser.yy`, `scaled_duration`).
            Self::Pair(paire) => {
                let numerateur = paire.0.as_fraction()?;
                let denominateur = paire.1.as_fraction()?;
                if numerateur.denominator() != 1 || denominateur.denominator() != 1 {
                    return None;
                }
                Fraction::try_new(numerateur.numerator(), denominateur.numerator())
            }
            _ => None,
        }
    }

    /// Représentation en réel.
    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(value) => Some(*value as f64),
            Self::Rational(value) => {
                let denominateur = value.denominator();
                if denominateur == 0 {
                    return None;
                }
                Some(value.numerator() as f64 / denominateur as f64)
            }
            Self::Real(value) => Some(*value),
            _ => None,
        }
    }
}

/// Lit tous les datums d'un texte (`(begin …)`, corps de fonction).
pub(crate) fn read_all(text: &str) -> Result<Vec<Value>, String> {
    let mut lecteur = Reader::new(text);
    let mut valeurs = Vec::new();
    loop {
        lecteur.saute();
        if lecteur.au_bout() {
            return Ok(valeurs);
        }
        valeurs.push(lecteur.datum()?);
    }
}

struct Reader {
    caracteres: Vec<char>,
    position: usize,
}

impl Reader {
    fn new(texte: &str) -> Self {
        Self {
            caracteres: texte.chars().collect(),
            position: 0,
        }
    }

    fn au_bout(&self) -> bool {
        self.position >= self.caracteres.len()
    }

    fn courant(&self) -> Option<char> {
        self.caracteres.get(self.position).copied()
    }

    fn suivant(&self) -> Option<char> {
        self.caracteres.get(self.position + 1).copied()
    }

    /// Saute espaces, commentaires `;` et blocs `#|…|#` (R7RS §2.2).
    fn saute(&mut self) {
        loop {
            while self.courant().is_some_and(char::is_whitespace) {
                self.position += 1;
            }
            match (self.courant(), self.suivant()) {
                (Some(';'), _) => {
                    while self.courant().is_some_and(|c| c != '\n') {
                        self.position += 1;
                    }
                }
                (Some('#'), Some('|')) => self.saute_bloc(),
                (Some('#'), Some(';')) => {
                    self.position += 2;
                    self.saute();
                    let _ = self.datum();
                }
                _ => return,
            }
        }
    }

    fn saute_bloc(&mut self) {
        let mut profondeur = 0usize;
        while !self.au_bout() {
            match (self.courant(), self.suivant()) {
                (Some('#'), Some('|')) => {
                    profondeur += 1;
                    self.position += 2;
                }
                (Some('|'), Some('#')) => {
                    profondeur = profondeur.saturating_sub(1);
                    self.position += 2;
                    if profondeur == 0 {
                        return;
                    }
                }
                _ => self.position += 1,
            }
        }
    }

    fn datum(&mut self) -> Result<Value, String> {
        self.saute();
        let Some(character) = self.courant() else {
            return Err("datum Scheme manquant".to_owned());
        };
        match character {
            '(' => self.liste(),
            ')' => Err("parenthèse fermante inattendue".to_owned()),
            '\'' => {
                self.position += 1;
                let valeur = self.datum()?;
                Ok(liste(vec![Value::Symbol("quote".to_owned()), valeur]))
            }
            '`' => {
                self.position += 1;
                let valeur = self.datum()?;
                Ok(liste(vec![Value::Symbol("quasiquote".to_owned()), valeur]))
            }
            ',' => {
                self.position += 1;
                let symbole = if self.courant() == Some('@') {
                    self.position += 1;
                    "unquote-splicing"
                } else {
                    "unquote"
                };
                let valeur = self.datum()?;
                Ok(liste(vec![Value::Symbol(symbole.to_owned()), valeur]))
            }
            '"' => self.chaine(),
            '#' => self.diese(),
            _ => self.atome(),
        }
    }

    fn liste(&mut self) -> Result<Value, String> {
        // `(` déjà consommée.
        self.position += 1;
        let mut elements = Vec::new();
        let mut queue = Value::Nil;
        loop {
            self.saute();
            match self.courant() {
                Some(')') => {
                    self.position += 1;
                    break;
                }
                None => return Err("liste Scheme non fermée".to_owned()),
                Some('.') if self.suivant().is_some_and(char::is_whitespace) => {
                    // Paire pointue `(a . b)` (R7RS §6.4).
                    self.position += 1;
                    queue = self.datum()?;
                    self.saute();
                    if self.courant() != Some(')') {
                        return Err("paire pointue non fermée".to_owned());
                    }
                    self.position += 1;
                    break;
                }
                _ => elements.push(self.datum()?),
            }
        }
        let mut resultat = queue;
        for element in elements.into_iter().rev() {
            resultat = Value::Pair(Rc::new((element, resultat)));
        }
        Ok(resultat)
    }

    fn chaine(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut texte = String::new();
        while let Some(character) = self.courant() {
            self.position += 1;
            match character {
                '"' => return Ok(Value::Str(texte)),
                '\\' => {
                    let Some(echappe) = self.courant() else {
                        break;
                    };
                    self.position += 1;
                    texte.push(match echappe {
                        'n' => '\n',
                        't' => '\t',
                        '"' => '"',
                        '\\' => '\\',
                        autre => autre,
                    });
                }
                autre => texte.push(autre),
            }
        }
        Err("chaîne Scheme non fermée".to_owned())
    }

    fn diese(&mut self) -> Result<Value, String> {
        self.position += 1;
        match self.courant() {
            Some('#') => {
                // `##t` : le `#` de LilyPond puis le `#t` de Scheme.
                self.diese()
            }
            Some('(') => {
                // Vecteur `#(…)` : lire la liste puis la convertir.
                let liste = self.liste()?;
                Ok(Value::Vector(Rc::new(
                    liste.into_list().ok_or("vecteur pointu")?,
                )))
            }
            Some('\\') => {
                self.position += 1;
                let Some(character) = self.courant() else {
                    return Err("caractère Scheme manquant".to_owned());
                };
                self.position += 1;
                let mut nom = String::from(character);
                while self.courant().is_some_and(|c| c.is_alphanumeric()) {
                    nom.push(self.courant().unwrap());
                    self.position += 1;
                }
                Ok(Value::Char(match nom.as_str() {
                    "space" => ' ',
                    "newline" => '\n',
                    "tab" => '\t',
                    _ => nom.chars().next().unwrap_or(character),
                }))
            }
            Some('{') => {
                // Musique embarquée : `#{ … #}`, transmise brute.
                self.position += 1;
                let début = self.position;
                let mut profondeur = 1usize;
                while !self.au_bout() {
                    match (self.courant(), self.suivant()) {
                        (Some('#'), Some('{')) => {
                            profondeur += 1;
                            self.position += 2;
                        }
                        (Some('#'), Some('}')) => {
                            profondeur -= 1;
                            self.position += 2;
                            if profondeur == 0 {
                                let fin = self.position - 2;
                                return Ok(Value::Embedded(
                                    self.caracteres[début..fin].iter().collect(),
                                ));
                            }
                        }
                        _ => self.position += 1,
                    }
                }
                Err("musique embarquée non fermée".to_owned())
            }
            Some('t' | 'T' | 'f' | 'F') => {
                let mot: String = self.caracteres[self.position..]
                    .iter()
                    .take(5)
                    .collect::<String>()
                    .to_ascii_lowercase();
                if mot.starts_with("true") {
                    self.position += 4;
                    Ok(Value::Bool(true))
                } else if mot.starts_with("false") {
                    self.position += 5;
                    Ok(Value::Bool(false))
                } else {
                    self.position += 1;
                    Ok(Value::Bool(!matches!(
                        self.caracteres[self.position - 1],
                        'f' | 'F'
                    )))
                }
            }
            _ => self.atome(),
        }
    }

    fn atome(&mut self) -> Result<Value, String> {
        let début = self.position;
        while let Some(character) = self.courant() {
            if character.is_whitespace()
                || matches!(
                    character,
                    '(' | ')' | '"' | '\'' | '`' | ',' | ';' | '{' | '}'
                )
            {
                break;
            }
            self.position += 1;
        }
        let texte: String = self.caracteres[début..self.position].iter().collect();
        if texte.is_empty() {
            return Err("atome Scheme vide".to_owned());
        }
        Ok(atom_value(&texte))
    }
}

/// Valeur d'un atome non délimité : nombre, rationnel ou symbole.
fn atom_value(texte: &str) -> Value {
    if let Ok(entier) = texte.parse::<i64>() {
        return Value::Int(entier);
    }
    if let Some((numerateur, denominateur)) = texte.split_once('/') {
        if let (Ok(numerateur), Ok(denominateur)) =
            (numerateur.parse::<i64>(), denominateur.parse::<i64>())
        {
            if let Some(fraction) = Fraction::try_new(numerateur, denominateur) {
                return Value::Rational(fraction);
            }
        }
    }
    if texte.contains('.') || texte.contains('e') || texte.contains('E') {
        if let Ok(reel) = texte.parse::<f64>() {
            return Value::Real(reel);
        }
    }
    Value::Symbol(texte.to_owned())
}

/// Valeur d'un nombre écrit en texte (`string->number`), ou `None`.
pub(crate) fn nombre_depuis_texte(texte: &str) -> Option<Value> {
    match atom_value(texte) {
        valeur @ (Value::Int(_) | Value::Rational(_) | Value::Real(_)) => Some(valeur),
        _ => None,
    }
}

/// Construit une liste propre à partir d'éléments.
pub(crate) fn liste(elements: Vec<Value>) -> Value {
    let mut resultat = Value::Nil;
    for element in elements.into_iter().rev() {
        resultat = Value::Pair(Rc::new((element, resultat)));
    }
    resultat
}
