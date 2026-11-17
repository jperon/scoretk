\version "2.24.0"
\language "english"

\header {
  title = "Repeat Test"
  subtitle = "Volta skipping entire score"
}

staffOneVoiceOne = \fixed c' {
  \time 4/4
  \tempo 4 = 400 e1 ^\markup { "no playback" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
