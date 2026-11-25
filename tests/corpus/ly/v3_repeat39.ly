\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Repeat Test" }
  workTitle = "Repeat Test"
  subtitle = "Alternating Volta"
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "repeat39"
  instrument = "Alternating Volta"
  scoretkCreditA = "repeat39"
  scoretkCreditB = "Alternating Volta"
  scoretkTboxA = "0|1-3   2, 4-5   2-3   2, 4-5   2, 6"
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
  \clef treble \time 4/4
  \tempo "♩ = 400" 4 = 400
  c1 ^\markup { "1-3   2, 4-5   2-3   2, 4-5   2, 6" } |
  \repeat volta 3 {
    d1 |
  }
  \alternative {
    {
      e1 |
    }
    {
      f1 |
      g1 |
    }
  }
  c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
