\version "2.24.0"
\language "english"

\header {
  title = "repeat11"
  subtitle = "Volta after to coda"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 c'1 ^\markup { "1,2,3,4   2,3   5,6,7,8   2   9,10" } |
  \repeat volta 2 {
    b1 |
    a1 |
  }
  \alternative {
    {
      g1 |
    }
    {
      f1 |
    }
  }
  e1 |
  d1 |
  c1 |
  \break
  d'1 |
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
