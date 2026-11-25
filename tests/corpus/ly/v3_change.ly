\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Instrument Change"
  workTitle = "Instrument Change"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
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
  c'2 c'2 |
  c'2 ^\markup { "Instrument" } c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 |
  c'2 c'2 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute" shortInstrumentName = "Fl." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "Oboe" shortInstrumentName = "Ob." } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
  >>
  \layout { }
}
