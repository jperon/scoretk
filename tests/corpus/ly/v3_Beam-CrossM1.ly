\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Beam Test Cross Measure" }
  workTitle = "Beam Test Cross Measure"
  subtitle = "Recreation of beam"
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Recreation of beam"
  instrument = "Beam Test-Cross Measure"
  scoretkCreditA = "Recreation of beam"
  scoretkCreditB = "Beam Test-Cross Measure"
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
  \break
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" midiInstrument = "choir aahs" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
