\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07626457)
\header {
  title = "repeat6"
  subtitle = "simple volta"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
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
  \clef treble \time 4/4
  \tempo 4 = 400 c1 ^\markup { "1,2,3   2   4,5,6" } |
  \repeat volta 2 {
    d1 |
  }
  \alternative {
    {
      e1 |
    }
    {
      f1 |
    }
  }
  g1 |
  c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
