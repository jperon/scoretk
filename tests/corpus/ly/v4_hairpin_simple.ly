\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  \tempo 4 = 120 b,4 _\p \< c4 d4 e4 _\f \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Steel Guitar" shortInstrumentName = "s.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
