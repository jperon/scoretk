\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c4-.-- \=1( \< d4-.-- e4-.-- f4 \=3( \fermata |
  \time 3/4
  c4-.-- \=1) d4-.-- e4-.-- |
  g2 \! g4 \=3) |
  b4-.-- c'4-.-- d'4 \> ^\markup { "simile" } |
  c'4 b4 a4 |
  g4 f4 e4 |
  d4 \! c2 _\p |
  d2 e4~ |
  e4 f2 |
  g2.~ ^\markup { "C" } ^\markup { "G7" } |
  g4~ g4 r4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
