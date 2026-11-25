\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
  scoretkCreditA = "Local Timesig 1"
  scoretkCreditB = "MuseScore testfile"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 15.000\mm
  right-margin = 15.000\mm
  top-margin = 15.000\mm
  bottom-margin = 15.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \key c \major \time 9/8
  R1*9/8 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \key c \major \time 3/4 \set Timing.measureLength = #9/8
  R1*9/8 \bar "|."
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
