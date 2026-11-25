\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\paper {
  paper-width = 210.05800\mm
  paper-height = 296.92600\mm
  left-margin = 15.00000\mm
  right-margin = 15.00000\mm
  top-margin = 15.00000\mm
  bottom-margin = 15.00000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  \break
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  \break
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } |
  g1*1/2:32 ^\markup { "scoretk-tremolo-start" }
  e'1*1/2:32 ^\markup { "scoretk-tremolo-stop" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
  >>
  \layout { }
}
