\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07626457)
\header {
  title = \markup { \fontsize #24.00000 "repeat7" }
  subtitle = "D.C. al Fine"
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "repeat7"
  instrument = "D.C. al Fine"
  scoretkCreditA = "repeat7"
  scoretkCreditB = "D.C. al Fine"
  scoretkTboxA = "0|1,2,3,4,5,6   1,2,3"
}

\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  \tempo "♩ = 400" 4 = 400 c1 ^\markup { "1,2,3,4,5,6   1,2,3" } |
  d1 |
  \mark \markup { "Fine" } e1 |
  f1 |
  g1 |
  \mark \markup { "D.C. al Fine" } c'1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
