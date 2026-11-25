\version "2.24.0"
\language "english"

\header {
  title = "Repeat Test"
  workTitle = "Repeat Test"
  subtitle = "Force Final Repeat should reset when crossing to next section."
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo 4 = 400
  c1 ^\markup { "1,2,3,4   2,3,4   1,2   5   |   1,2,3,4   2,3,4   2   5" } |
  \repeat volta 3 {
    d1 |
  }
  \alternative {
    {
      e1 |
      f1 |
    }
    {
      \mark \markup { "D.C." } g1 ^\markup { "3x" } \bar "|."
    }
  }
  \section
  g1 |
  \repeat volta 3 {
    f1 |
  }
  \alternative {
    {
      e1 |
      d1 |
    }
    {
      c1 ^\markup { "3x" } \bar "|."
    }
  }
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
