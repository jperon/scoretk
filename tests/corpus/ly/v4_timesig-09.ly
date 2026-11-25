\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \break
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } |
  \break
  \repeat tremolo 4 { g8 e'8 } |
  \repeat tremolo 4 { g8 e'8 } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
