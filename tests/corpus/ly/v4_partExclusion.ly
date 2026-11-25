\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Untitled score" }
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  c4 ^\markup { "Staff text" } c4 c4 ^\markup { "System text" } c4 |
  \clef bass
  c4 c4 c4 c4 |
  \ottava 1 c,4 c,4 c,4 \ottava 0 c,4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." midiInstrument = "flute" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
