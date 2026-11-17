\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  b4~ b4 r2 |
  \appoggiatura { b8 } b4 r4 \afterGrace b4~ { b8 } r4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
