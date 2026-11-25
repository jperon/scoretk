\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Title"
  workTitle = "Title"
  composer = "Composer on page 1"
  poet = "Lyricist on page 1"
  copyright = "Copyright"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  scoretkCreditA = "Title on page 1"
  scoretkCreditB = "Composer on page 1"
  scoretkCreditC = "Lyricist on page 1"
  scoretkCreditD = "Title in vbox on page 2"
  scoretkCreditE = "Text on page 2"
  scoretkCreditF = "Title in vbox on page 3"
  scoretkCreditG = "Part name on page 3"
  scoretkTboxA = "0|Above"
  scoretkTboxB = "0|Below"
  scoretkTboxC = "1|Above 2"
  scoretkTboxD = "1|Below 2"
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
  R1 ^\markup { "Above" } _\markup { "Below" } |
  R1*2 |
  g1 ^\markup { "Staff Text" } _\markup { "Staff Text" } |
  g1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
