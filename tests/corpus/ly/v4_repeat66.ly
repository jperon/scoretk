\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold "Repeat Test" }
  workTitle = "Repeat Test"
  subtitle = "Force Final Repeat should reset when crossing to next section."
  subsubtitle = "repeat66"
  instrument = "Force Final Repeat should reset when crossing to next section."
  scoretkCreditA = "repeat66"
  scoretkCreditB = "Force Final Repeat should reset when crossing to next section."
  scoretkTboxA = "0|1,2,3,4   2,3,4   1,2   5   |   1,2,3,4   2,3,4   2   5"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo "♩ = 400" 4 = 400
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
