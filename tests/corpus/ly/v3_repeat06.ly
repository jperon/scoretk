\version "2.24.0"
\language "english"

\header {
  title = "repeat6"
  subtitle = "simple volta"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 c1 ^\markup { "1,2,3   2   4,5,6" } |
  \repeat volta 2 {
    d1 |
  }
  \alternative {
    {
      e1 |
    }
    {
      f1 |
    }
  }
  g1 |
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
