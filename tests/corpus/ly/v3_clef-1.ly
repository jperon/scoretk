\version "2.24.0"
\language "english"

staffOneVoiceOne = \fixed c' {
  \clef treble \clef bass \time 3/4
  c,4 d,4 e,4 |
  e,4 r2 |
  R2. |
  R2. |
  R2. |
  R2. |
  R2. \bar "|."
}

staffTwoVoiceOne = \fixed c' {
  \clef bass \clef treble \time 3/4
  c'4 d'4 e'4 |
  R2. |
  R2. |
  R2. |
  R2. |
  R2. |
  R2. \bar "|."
}

\score {
  <<
    \new PianoStaff \with { instrumentName = "Piano" } <<
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
