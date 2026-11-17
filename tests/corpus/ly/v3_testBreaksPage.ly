\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  R1 ^\markup { "First system (no frames above)" } |
  \break
  R1 ^\markup { "After page break" } |
  R1 _\markup { "Tframe" } ^\markup { "After page break in vframe" } |
  R1 ^\markup { "After page break in tframe" } |
  R1 ^\markup { "After page break in hframe" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
