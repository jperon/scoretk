\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \key c \major \time 4/4
  \tempo 4 = 120 g4 _\f \< a4 c'4 d'4 |
  c'1 _\mf \! \bar "||"
  g4 _\f \> a4 c'4 d'4 |
  c'1 _\ff \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Clean Guitar" shortInstrumentName = "el.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
