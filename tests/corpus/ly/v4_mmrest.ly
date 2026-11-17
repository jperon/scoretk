\version "2.24.0"
\language "english"

\header {
  title = "Title"
  composer = "Composer"
}

staffOneVoiceOne = \fixed c' {
  \time 4/4
  R1 |
  R1*3 |
  R1 |
  R1 |
  \break
  R1*4 |
  R1 |
  R1 |
  R1 |
  \break
  R1*4 |
  R1 |
  R1 |
  R1 |
  \break
  R1*4 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
