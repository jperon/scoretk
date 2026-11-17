\version "2.24.0"
\language "english"

\header {
  title = "repeat7"
  subtitle = "D.C. al Fine"
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 c1 ^\markup { "1,2,3,4,5,6   1,2,3" } |
  d1 |
  e1 |
  \mark \markup { "Fine" } f1 |
  g1 |
  \mark \markup { "D.C. al Fine" } c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
