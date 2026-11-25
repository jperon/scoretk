\version "2.24.0"
\language "english"

#(set-global-staff-size 20.07501265)
\header {
  title = \markup { \bold \fontsize #24.00000 "Hairpin Dynamics" }
  workTitle = "Hairpin Dynamics"
  subtitle = "MuseScore testfile"
  workNumber = "MuseScore testfile"
  composer = \markup { \fontsize #12.00000 "Leon Vinken" }
  poet = \markup { \fontsize #12.00000 "" }
  scoretkTitleFontSize = "24.00000"
  scoretkComposerFontSize = "12.00000"
  scoretkLyricistFontSize = "12.00000"
  subsubtitle = "Hairpin Dynamics"
  instrument = "MuseScore testfile"
  piece = "Leon Vinken"
  scoretkCreditA = "Hairpin Dynamics"
  scoretkCreditB = "MuseScore testfile"
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
  \set Staff.midiMinimumVolume = #0.629921
  \set Staff.midiMaximumVolume = #0.629921 g4 _\mf \< a4 b4 c'4 |
  g4 \! \< a4 b4 c'4 |
  \break
  g4 \! \< a4 b4 c'4 |
  g4 \! \< a4 b4 c'4 |
  \break
  g4 \! \cresc a4 b4 c'4 |
  \break
  R1 \! \bar "|."
}

\score {
  <<
    \new Staff = "staffOne" \with { instrumentName = "Voice" shortInstrumentName = "Vo." } <<
      \new Voice = "staffOneVoiceOne" { \staffOneVoiceOne }
      \new Lyrics \lyricsto "staffOneVoiceOne" {
        dyn hairpin \skip 1 \skip 1 combined \skip 1
        \skip 1 \skip 1 \skip 1 \skip 1 \skip 1 \skip 1
        \skip 1 \skip 1 \skip 1 \skip 1 \skip 1 \skip 1
        \skip 1 \skip 1
      }
    >>
  >>
  \layout { }
}
