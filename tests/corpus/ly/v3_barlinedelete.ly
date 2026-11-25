\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Title" }
  workTitle = "Title"
  composer = \markup { \fontsize #12.00000 "Composer" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
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
  \tempo "Tempo text" 4 = 120 c'4 e'4 g'4 e'4 \bar "||"
  c'4 g4 c'4 e'4 |
  \mark \markup { "1" } g'4 r4 r2 |
  R1 |
  \break
  R1 \bar "||"
  R1 ^\markup { "Staff text" } |
  R1 \fermata |
  R1 ^\markup { "System text" } |
  \time 3/4
  R2. |
  R2. \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
