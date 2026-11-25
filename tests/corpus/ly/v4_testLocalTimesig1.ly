\version "2.24.0"
\language "english"

\header {
  title = \markup { \bold "Untitled score" }
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
  subsubtitle = "Local Timesig 1"
  instrument = "MuseScore testfile"
  scoretkCreditA = "Local Timesig 1"
  scoretkCreditB = "MuseScore testfile"
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
  \clef treble \key c \major \time 9/8
  R1*9/8 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \key c \major \time 3/4 \set Timing.measureLength = #9/8
  R1*9/8 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano 1" shortInstrumentName = "Pno. 1" midiInstrument = "acoustic grand" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "Piano 2" shortInstrumentName = "Pno. 2" midiInstrument = "acoustic grand" } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
  >>
  \layout { }
}
