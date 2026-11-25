\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  \tempo 4 = 120 b,4 _\p \< c4 d4 e4 _\f \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Steel Guitar" shortInstrumentName = "s.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
