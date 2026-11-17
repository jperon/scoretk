\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \key c \major \time 4/4
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano 1" shortInstrumentName = "Pno. 1" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "Piano 2" shortInstrumentName = "Pno. 2" } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
  >>
  \layout { }
}
