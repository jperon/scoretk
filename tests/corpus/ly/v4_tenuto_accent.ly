\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  f4---> r4 r2 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef bass \time 4/4
  R1 \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Staff = "staffOne" <<
        \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      >>
      \new Staff = "staffTwo" <<
        \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
      >>
    >>
  >>
  \layout { }
}
