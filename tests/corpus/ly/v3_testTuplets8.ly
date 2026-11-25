\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Tuplets 8" }
  workTitle = "Tuplets 8"
  subtitle = "MuseScore testfile"
  opus = "MuseScore testfile"
  composer = \markup { \fontsize #12.00000 "Leon Vinken" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Tuplets 8"
  instrument = "MuseScore testfile"
  piece = "Leon Vinken"
  scoretkCreditA = "Tuplets 8"
  scoretkCreditB = "MuseScore testfile"
  scoretkCreditC = "Leon Vinken"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
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
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." midiInstrument = "choir aahs" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
