\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07626457)
\header {
  title = \markup { \fontsize #24.00000 "" }
  composer = \markup { \fontsize #12.00000 "" }
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
