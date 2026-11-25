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
  \ottava 1 \tempo 4 = 80 b4->\bendAfter #-3 \sustainOn cs'4-. \=4( b4-- \=4)
  \grace { b,32 } b4:16 \fermata \bar "||"
  \key e \major \time 3/4 \clef bass
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 \ottava 0
  <b fs'>4 \sustainOff _\p \< b4~-^ b4 |
  \key c \major \time 4/4 \clef treble
  b4 \! \startTrillSpan
  \repeat tremolo 4 { b16 \stopTrillSpan b16 \glissando } f4 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
