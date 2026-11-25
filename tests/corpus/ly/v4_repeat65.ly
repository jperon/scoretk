\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold "Repeat Test" }
  workTitle = "Repeat Test"
  subtitle = "Jump at volta end with end repeat interpretation order"
  subsubtitle = "repeat65"
  instrument = "Jump at volta end with end repeat interpretation order"
  scoretkCreditA = "repeat65"
  scoretkCreditB = "Jump at volta end with end repeat interpretation order"
  scoretkTboxA = "0|1,2,3,4   2,3,4   1,2   5"
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
  \tempo "♩ = 400" 4 = 400 c1 ^\markup { "1,2,3,4   2,3,4   1,2   5" } |
  \repeat volta 3 {
    d1 |
  }
  \alternative {
    {
      e1 |
      \mark \markup { "D.C." } f1 |
    }
    {
      g1 ^\markup { "3x" } \bar "|."
    }
  }
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
