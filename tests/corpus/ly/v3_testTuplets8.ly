\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Tuplets 8"
  workTitle = "Tuplets 8"
  subtitle = "MuseScore testfile"
  workNumber = "MuseScore testfile"
  composer = "Leon Vinken"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  scoretkCreditA = "Tuplets 8"
  scoretkCreditB = "MuseScore testfile"
  scoretkCreditC = "Leon Vinken"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 2/4
  \tuplet 3/2 {
    c'4 b4
    \tuplet 5/4 {
    a16 g16 f16 e16 d16
    }
  }
  |
  \tuplet 3/2 {
    c'4
    \tuplet 5/4 {
    b16 a16 g16 f16 e16
    }
  d4
  }
  |
  \tuplet 3/2 {
    \tuplet 5/4 {
    c'16 b16 a16 g16 f16
    }
  e4 d4
  }
  \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
