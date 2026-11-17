\version "2.24.0"
\language "english"

\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer"
}

staffOneVoiceOne = \fixed c' {
  \time 4/4
  \tempo 4 = 88 R1 |
  \tempo "Andante" 4 = 92 R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
