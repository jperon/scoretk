\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = "Harmony 6"
  workTitle = "Harmony 6"
  subtitle = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  workNumber = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  composer = "Leon Vinken"
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  scoretkCreditA = "Harmony 6"
  scoretkCreditB = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  scoretkCreditC = "Leon Vinken"
}

\paper {
  paper-width = 210.058\mm
  paper-height = 296.926\mm
  left-margin = 10.000\mm
  right-margin = 10.000\mm
  top-margin = 10.000\mm
  bottom-margin = 20.000\mm
}

staffOneVoiceOne = \fixed c' {
  \clef treble \time 4/4
  c'1 ^\markup { "Cm" } |
  c'1 ^\markup { "i" } |
  c'1 ^\markup { "m" } |
  \break
  c'1 ^\markup { "xyz" } |
  c'1 ^\markup { "xyz" } |
  c'1 ^\markup { "xyz" } |
  \break
  c'1 ^\markup { "<>&\"" } |
  c'1 ^\markup { "<>&\"" } |
  c'1 ^\markup { "<>&\"" } \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        Normal Roman Nashville Normal Roman Nashville
        Normal Roman Nashville
      }
    >>
  >>
  \layout { }
}
