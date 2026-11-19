\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \transposition c \key c \major \time 4/4
  \tempo 4 = 120 c'4 _\mf \< d'4 e'4 f'4 _\p |
  g'4 a'4 _\f b'4 r4 \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Steel Guitar" shortInstrumentName = "s.guit." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
