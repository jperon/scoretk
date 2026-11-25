\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \fontsize #24.00000 "" }
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Vframe"
  scoretkCreditA = "Vframe"
  scoretkTboxA = "2|Tframe"
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
  R1 ^\markup { "First system (no frames above)" } |
  \break
  R1 ^\markup { "After page break" } |
  R1 _\markup { "Tframe" } ^\markup { "After page break in vframe" } |
  R1 ^\markup { "After page break in tframe" } |
  R1 ^\markup { "After page break in hframe" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." midiInstrument = "choir aahs" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
