\version "2.24.0"
\language "english"

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

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
