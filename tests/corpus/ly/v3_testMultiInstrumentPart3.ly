\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "clarinet scales" }
  workTitle = "clarinet scales"
  composer = \markup { \fontsize #12.00000 "" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "clarinet scales"
  scoretkCreditA = "clarinet scales"
}

\paper {
  paper-width = 215.90000\mm
  paper-height = 279.40000\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \transposition a \key c \major \time 4/4
  ef4 ^\markup { "A Clarinet" } f4 g4 af4 |
  bf4 c'4 d'4 ef'4 |
  \transposition bf \key c \major
  ef4 ^\markup { "B♭ Clarinet" } f4 g4 af4 |
  bf4 c'4 d'4 ef'4 |
  R1 |
  R1 |
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "A Clarinet" shortInstrumentName = "A Cl." midiInstrument = "clarinet" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
