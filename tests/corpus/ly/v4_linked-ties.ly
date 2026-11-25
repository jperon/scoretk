\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  b4~ b4 r2 |
  \appoggiatura { b8~ } b4 r4 \afterGrace 1 b4~ { b8 } r4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
