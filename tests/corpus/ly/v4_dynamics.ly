\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  d'4 _\p d'4 d'4 d'4 |
  d'4 \< d'4 d'4 d'4 |
  d'4 _\f \! d'4 d'4 d'4 |
  d'4 \< d'4 d'4 d'4 |
  d'4 _\pp \! d'4 d'4 d'4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Violin" shortInstrumentName = "Vln." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
