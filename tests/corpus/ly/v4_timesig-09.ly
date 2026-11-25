\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  \break
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 |
  \break
  g1*1/2:32 e'1*1/2:32 |
  g1*1/2:32 e'1*1/2:32 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
