\version "2.24.0"
\language "english"

\header {
  title = "Repeat Test"
  subtitle = "Alternating Volta"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 c1 ^\markup { "1-3   2, 4-5   2-3   2, 4-5   2, 6" } |
  \repeat volta 3 {
    d1 |
  }
  \alternative {
    {
      e1 |
    }
    {
      f1 |
      g1 |
    }
  }
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
