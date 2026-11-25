\version "2.24.0"
\language "english"

\header {
  title = "Untitled score"
  workTitle = "Untitled score"
  subtitle = "Subtitle"
  composer = "Composer / arranger"
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
  \clef treble \time 4/4
  gs8.-> r16 r4.
  \appoggiatura { gs8 } \appoggiatura { gs8 } \appoggiatura { gs8 } gs8 \=1(
  r8 \=1) gs8 |
  <fs a c'>8.-> r16 r4 r2 |
  <g bs>8-> g8 g8 g8 r2 |
  b2\turn c'2\trill \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Piano" shortInstrumentName = "Pno." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        \skip 1 lyric \skip 1 \skip 1 \skip 1 \skip 1
        \skip 1 \skip 1 \skip 1 \skip 1
      }
    >>
  >>
  \layout { }
}
