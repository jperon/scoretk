\version "2.24.0"
\language "english"

\header {
  title = "Instrument Change"
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
