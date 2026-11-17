\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'1 |
  d'1 |
  \break
  R1 |
  R1 |
  \break
  e'1 |
  f'1 |
  \break
  e'1 |
  f'1 \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef treble \time 4/4
  e'1 |
  f'1 |
  \break
  e'1 |
  f'1 |
  \break
  R1 |
  R1 |
  \break
  c'1 |
  c'1 \bar "|."
}

staffThreeVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'1 |
  d'1 |
  \break
  e'1 |
  f'1 |
  \break
  e'1 |
  d'1 |
  \break
  R1 |
  R1 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Flute 1" shortInstrumentName = "Fl. 1" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
    >>
    \new Staff = "staffTwo" \with { instrumentName = "Flute 2" shortInstrumentName = "Fl. 2" } <<
      \new Voice = "staffTwoVoiceOne" { \staffTwoVoiceOne }
    >>
    \new Staff = "staffThree" \with { instrumentName = "Flute 3" shortInstrumentName = "Fl. 3" } <<
      \new Voice = "staffThreeVoiceOne" { \staffThreeVoiceOne }
    >>
  >>
  \layout { }
}
