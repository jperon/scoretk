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

//! Évaluateur Scheme minimal pour les valeurs du lecteur LilyPond.
//!
//! LilyPond évalue le Scheme embarqué avec Guile (`lily/parse-scm.cc:206-307`,
//! `lily/lexer.ll:407-441`) ; le lecteur ne peut pas embarquer Guile, mais il
//! peut lire et évaluer un sous-ensemble du langage pour les **valeurs** dont
//! il a besoin : nombres, booléens, symboles, listes, durées et fonctions
//! musicales. C'est le niveau B de `docs/agents/lilypond-scheme.md`.
//!
//! Sémantique : R7RS-small, §4.1.2 (littéraux), §4.1.4 (`lambda`), §4.1.5
//! (conditionnels), §4.1.6 (`set!`), §4.2.1 (conditionnels dérivés), §4.2.2
//! (`let`), §4.2.8 (quasiquotation), §5.3 (définitions), §6.2 (nombres), §6.4
//! (paires et listes), §6.5–6.8 (symboles, caractères, chaînes, vecteurs).
//! Les nombres exacts sont des rationnels ([`Fraction`]) ; les erreurs
//! deviennent un message que l'appelant transforme en avertissement : le
//! lecteur ne doit jamais échouer sur du Scheme (`lib.rs:48`).

mod datum;
mod eval;
mod primitives;

pub(crate) use datum::{read_all, Value};

use scoretk_core::model::Fraction;

/// Environnement d'évaluation persistant d'une lecture.
///
/// Les `#(define …)` du fichier y sont enregistrés : une expression ultérieure
/// peut utiliser les fonctions et variables définies plus haut, comme Guile
/// dans le module du parseur (`lily/parse-scm.cc:288-303`).
pub(crate) struct Evaluator {
    global: eval::Env,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    /// Environnement neuf, avec les primitives Scheme et LilyPond.
    pub(crate) fn new() -> Self {
        Self {
            global: eval::global_env(),
        }
    }

    /// Évalue le texte d'un datum Scheme (`#(- 4)`, `(let ((x 4)) x)`).
    pub(crate) fn eval(&mut self, text: &str) -> Result<Value, String> {
        let forms = read_all(text)?;
        let mut valeur = Value::Unspecified;
        for form in forms {
            valeur = eval::eval(&form, &mut self.global, 0)?;
        }
        Ok(valeur)
    }

    /// Évalue un datum et le renvoie comme nombre (réel).
    pub(crate) fn number(&mut self, text: &str) -> Option<f64> {
        self.eval(text).ok().and_then(|valeur| valeur.as_f64())
    }

    /// Évalue un datum et le renvoie comme entier.
    pub(crate) fn integer(&mut self, text: &str) -> Option<i64> {
        match self.eval(text).ok()? {
            Value::Int(value) => Some(value),
            autre => autre.as_f64().and_then(|value| {
                (value.fract() == 0.0 && value.is_finite()).then_some(value as i64)
            }),
        }
    }

    /// Évalue un datum et le renvoie comme fraction exacte.
    pub(crate) fn fraction(&mut self, text: &str) -> Option<Fraction> {
        self.eval(text).ok().and_then(|valeur| valeur.as_fraction())
    }

    /// Évalue un datum et le renvoie comme booléen.
    ///
    /// Seul `#f` est faux (R7RS §6.3) ; une expression qui ne rend pas un
    /// booléen n'est pas acceptée ici, pour ne pas changer le sens des
    /// propriétés qui attendent un booléen.
    pub(crate) fn boolean(&mut self, text: &str) -> Option<bool> {
        match self.eval(text).ok()? {
            Value::Bool(value) => Some(value),
            _ => None,
        }
    }

    /// Lit un datum et le renvoie comme nom de symbole.
    ///
    /// `#'UP` est le datum `(quote UP)` (évalué) ; `#UP` est déjà le symbole
    /// et n'est pas évalué : c'est ainsi que LilyPond écrit une valeur
    /// symbolique sans quotation dans `\override`.
    pub(crate) fn symbol(&mut self, text: &str) -> Option<String> {
        let formes = read_all(text).ok()?;
        if let Some(Value::Symbol(nom)) = formes.last() {
            return Some(nom.clone());
        }
        match self.eval(text).ok()? {
            Value::Symbol(nom) => Some(nom),
            _ => None,
        }
    }

    /// Évalue les formes d'un corps de fonction avec des liaisons locales,
    /// dans une portée enfant de l'environnement global.
    ///
    /// Les `#(define …)` déjà rencontrés dans le fichier restent visibles
    /// (Guile : le module du parseur, `lily/parse-scm.cc:288-303`).
    pub(crate) fn eval_with(
        &mut self,
        bindings: &[(String, Value)],
        forms: &[Value],
    ) -> Result<Value, String> {
        let mut portee = eval::child_env(&self.global);
        for (nom, valeur) in bindings {
            eval::define_value(&mut portee, nom.clone(), valeur.clone());
        }
        let mut resultat = Value::Unspecified;
        for form in forms {
            resultat = eval::eval(form, &mut portee, 0)?;
        }
        Ok(resultat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evalue(texte: &str) -> Result<Value, String> {
        Evaluator::new().eval(texte)
    }

    #[test]
    fn litteraux_et_arithmetique() {
        // R7RS §4.1.2, §6.2.
        assert_eq!(evalue("(+ 1 2 3)").unwrap(), Value::Int(6));
        assert_eq!(evalue("(- 4)").unwrap(), Value::Int(-4));
        assert_eq!(
            evalue("(* 2 3/4)").unwrap(),
            Value::Rational(Fraction::new(3, 2))
        );
        assert_eq!(
            evalue("(/ 1 4)").unwrap(),
            Value::Rational(Fraction::new(1, 4))
        );
        assert_eq!(
            evalue("(+ 1/4 1/4)").unwrap(),
            Value::Rational(Fraction::new(1, 2))
        );
        assert_eq!(evalue("(quotient 7 2)").unwrap(), Value::Int(3));
        assert_eq!(evalue("(modulo -7 2)").unwrap(), Value::Int(1));
        // LilyPond : `#t` est le symbole `t` (le lexer ne transmet que le
        // datum qui suit `#`), `##t` le booléen ; les deux sont vrais.
        assert_eq!(evalue("t").unwrap(), Value::Symbol("t".to_owned()));
        assert_eq!(evalue("#t").unwrap(), Value::Bool(true));
        assert_eq!(evalue("##t").unwrap(), Value::Bool(true));
    }

    #[test]
    fn definitions_et_liaisons() {
        let mut e = Evaluator::new();
        e.eval("(define x 4)").unwrap();
        assert_eq!(e.eval("(* x 2)").unwrap(), Value::Int(8));
        e.eval("(define (double n) (* n 2))").unwrap();
        assert_eq!(e.eval("(double 3)").unwrap(), Value::Int(6));
        assert_eq!(
            e.eval("(let ((a 1) (b 2)) (+ a b))").unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            e.eval("(let* ((a 1) (b (+ a 1))) b)").unwrap(),
            Value::Int(2)
        );
        e.eval("(set! x (+ x 1))").unwrap();
        assert_eq!(e.eval("x").unwrap(), Value::Int(5));
    }

    #[test]
    fn conditionnels() {
        assert_eq!(evalue("(if #t 1 2)").unwrap(), Value::Int(1));
        assert_eq!(evalue("(if #f 1 2)").unwrap(), Value::Int(2));
        assert_eq!(evalue("(cond (#f 1) (else 2))").unwrap(), Value::Int(2));
        assert_eq!(evalue("(and 1 2 3)").unwrap(), Value::Int(3));
        assert_eq!(evalue("(or #f 2)").unwrap(), Value::Int(2));
        assert_eq!(evalue("(not #f)").unwrap(), Value::Bool(true));
        assert_eq!(evalue("(when #t 1)").unwrap(), Value::Int(1));
        assert_eq!(evalue("(unless #f 2)").unwrap(), Value::Int(2));
    }

    #[test]
    fn listes_et_quasiquote() {
        assert_eq!(evalue("(length (list 1 2 3))").unwrap(), Value::Int(3));
        assert_eq!(
            evalue("(car '(a b))").unwrap(),
            Value::Symbol("a".to_owned())
        );
        assert_eq!(
            evalue("(cdr '(a b))").unwrap().into_list().unwrap(),
            vec![Value::Symbol("b".to_owned())]
        );
        assert_eq!(evalue("(apply + '(1 2 3))").unwrap(), Value::Int(6));
        assert_eq!(
            evalue("(map (lambda (n) (* n n)) '(1 2 3))")
                .unwrap()
                .into_list()
                .unwrap(),
            vec![Value::Int(1), Value::Int(4), Value::Int(9)]
        );
        assert_eq!(
            evalue("(iota 3)").unwrap().into_list().unwrap(),
            vec![Value::Int(0), Value::Int(1), Value::Int(2)]
        );
        assert_eq!(
            evalue("`(a ,(+ 1 2) ,@(list 4 5))")
                .unwrap()
                .into_list()
                .unwrap(),
            vec![
                Value::Symbol("a".to_owned()),
                Value::Int(3),
                Value::Int(4),
                Value::Int(5)
            ]
        );
        assert_eq!(
            evalue("(symbol->string 'cross)").unwrap(),
            Value::Str("cross".to_owned())
        );
        assert_eq!(
            evalue("(string-append \"a\" \"b\")").unwrap(),
            Value::Str("ab".to_owned())
        );
    }

    #[test]
    fn primitives_lilypond() {
        assert_eq!(
            evalue("(ly:make-moment 3 4)").unwrap(),
            Value::Rational(Fraction::new(3, 4))
        );
        assert_eq!(
            evalue("(ly:make-moment 1/8)").unwrap(),
            Value::Rational(Fraction::new(1, 8))
        );
        // `ly:set-option` et compagnie sont des effets de bord ignorés.
        assert_eq!(
            evalue("(ly:set-option 'warning-as-error #t)").unwrap(),
            Value::Unspecified
        );
        assert_eq!(evalue("(magstep 0)").unwrap(), Value::Real(1.0));
    }

    #[test]
    fn erreurs_signalees_sans_panique() {
        assert!(evalue("(car 1)").is_err());
        assert!(evalue("(inconnue 1)").is_err());
        assert!(evalue("(").is_err());
    }
}
