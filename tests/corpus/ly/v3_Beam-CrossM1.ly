\version "2.24.0"
\language "english"

\header {
  title = "Beam Test Cross Measure"
  workTitle = "Beam Test Cross Measure"
  subtitle = "Recreation of beam"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  r2 r4 r8 g8 |
  a8 b8 a8 c'8 a8 c'8 b8 g8 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
