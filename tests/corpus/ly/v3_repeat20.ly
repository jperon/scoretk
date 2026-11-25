\version "2.24.0"
\language "english"

\header {
  title = "repeat20"
  subtitle = "DS al Coda followed by DS1 al Fine"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400 \mark \markup { "segno" }
  c1 ^\markup { "1,2,3   1   4,5,6,7,8   5,6" } |
  \mark \markup { "To Coda" } d1 |
  e1 |
  \mark \markup { "D.S. al Coda" } f1 ^\markup { "coda" } |
  \break
  g1 |
  \mark \markup { "segno1" } a1 |
  \mark \markup { "Fine" } b1 |
  \mark \markup { "D.S.1 al Fine" } c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
