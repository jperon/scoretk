\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Beam Test Cross Measure"
  workTitle = "Beam Test Cross Measure"
  subtitle = "Recreation of beam"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  scoretkCreditA = "Recreation of beam"
  scoretkCreditB = "Beam Test-Cross Measure"
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
  r2 r4 r8 g8 |
  a8 b8 a8 c'8 a8 c'8 b8 g8 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  \break
  R1 |
  R1 |
  R1 |
  \break
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
