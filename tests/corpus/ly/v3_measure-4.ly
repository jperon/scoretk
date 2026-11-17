\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  g4 g4 \> g4 g4 |
  g4 g4 g4 g4 \! |
  g4 g4 g4 g4 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
