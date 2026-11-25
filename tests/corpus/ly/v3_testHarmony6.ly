\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Harmony 6" }
  workTitle = "Harmony 6"
  subtitle = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  opus = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  composer = \markup { \fontsize #12.00000 "Leon Vinken" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Harmony 6"
  instrument = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  piece = "Leon Vinken"
  scoretkCreditA = "Harmony 6"
  scoretkCreditB = "MuseScore testfile Chords using normal, Roman and Nashville notation containing recognized text, unrecognized plaintext and unrecognized text requiring HTML escape"
  scoretkCreditC = "Leon Vinken"
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
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." midiInstrument = "choir aahs" } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        Normal Roman Nashville Normal Roman Nashville
        Normal Roman Nashville
      }
    >>
  >>
  \layout { }
}
