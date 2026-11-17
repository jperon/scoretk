\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \time 4/4
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
