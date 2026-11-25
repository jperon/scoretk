\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
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
  \clef treble \time 4/4
  a4 b4 c'4 d'4 \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." midiInstrument = "choir aahs" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        A __ \skip 1 \skip 1 \skip 1
      }
    >>
  >>
  \layout { }
}
