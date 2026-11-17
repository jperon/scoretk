\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  \tempo 4 = 120 b4 _\p \< c'4 d'4 e'4 _\f \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Steel Guitar" shortInstrumentName = "s.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
