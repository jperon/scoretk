\version "2.24.0"
\language "english"

\header {
  title = "repeat20"
  subtitle = "DS al Coda followed by DS1 al Fine"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 c1 ^\markup { "1,2,3   1   4,5,6,7,8   5,6" } |
  d1 |
  e1 |
  f1 |
  \break
  g1 |
  a1 |
  b1 |
  c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
