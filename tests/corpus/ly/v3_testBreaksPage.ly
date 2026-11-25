\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
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
  R1 ^\markup { "First system (no frames above)" } |
  \break
  R1 ^\markup { "After page break" } |
  R1 _\markup { "Tframe" } ^\markup { "After page break in vframe" } |
  R1 ^\markup { "After page break in tframe" } |
  R1 ^\markup { "After page break in hframe" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
