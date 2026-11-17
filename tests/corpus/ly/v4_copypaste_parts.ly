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
  \tempo 4 = 80 b4->\bendAfter #-3 \sustainOn \ottava 1 cs'4-. \=4( b4-- \=4)
  \grace { b,32 } b4:16 \fermata \bar "||"
  \key e \major \time 3/4 \clef bass
  <b fs'>4 \sustainOff _\p \< \ottava 0 b4~-^ b4 |
  \key c \major \time 4/4 \clef treble
  b4 \! \startTrillSpan b2*1/2:32 \stopTrillSpan b2*1/2:32 \glissando f4 |
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
