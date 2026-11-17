\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  c'1 _\markup { "expr text" } |
  c'1 _\markup { "expr text" } |
  c'1 _\markup { "expr text" } |
  c'1 _\markup { "expr text" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
