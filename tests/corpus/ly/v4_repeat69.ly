\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Untitled score" }
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
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
  \clef treble \key c \major \time 4/4
  R1 |
  \mark \markup { "Segno" } R1 |
  \mark \markup { "D.S." } R1 |
  R1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \key c \major \time 4/4
  R1 |
  \mark \markup { "Segno" } R1 |
  \mark \markup { "D.S." } R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano 1" shortInstrumentName = "Pno. 1" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "Piano 2" shortInstrumentName = "Pno. 2" } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
  >>
  \layout { }
}
