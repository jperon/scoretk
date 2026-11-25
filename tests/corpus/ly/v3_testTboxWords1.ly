\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Title" }
  workTitle = "Title"
  composer = \markup { \fontsize #12.00000 "Composer on page 1" }
  poet = \markup { \fontsize #12.00000 "Lyricist on page 1" }
  copyright = "Copyright"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Title on page 1"
  instrument = "Composer on page 1"
  piece = "Lyricist on page 1"
  dedication = "Title in vbox on page 2"
  tagline = "Text on page 2"
  meter = "Title in vbox on page 3"
  breakbefore = "Part name on page 3"
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
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 10.00000\mm
  right-margin = 10.00000\mm
  top-margin = 10.00000\mm
  bottom-margin = 20.00000\mm
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
