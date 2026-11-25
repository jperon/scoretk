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
  \ottava 1 \tempo 4 = 80 b4->\bendAfter #-3 \sustainOn cs'4-. \=4( b4-- \=4)
  \grace { b,32 } b4:16 \fermata \bar "||"
  \key e \major \time 3/4 \clef bass
  \set Staff.midiMinimumVolume = #0.385827
  \set Staff.midiMaximumVolume = #0.385827 \ottava 0
  <b fs'>4 \sustainOff _\p \< b4~-^ b4 |
  \key c \major \time 4/4 \clef treble
  b4 \! \startTrillSpan
  b2*1/2:32 ^\markup { "scoretk-tremolo-start" } \stopTrillSpan
  b2*1/2:32 ^\markup { "scoretk-tremolo-stop" } \glissando f4 |
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
