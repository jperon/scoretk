\version "2.24.0"
\language "english"

\header {
  title = "Hairpin Dynamics"
  workTitle = "Hairpin Dynamics"
  subtitle = "MuseScore testfile"
  workNumber = "MuseScore testfile"
  composer = "Leon Vinken"
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
  g4 _\mf \< a4 b4 c'4 |
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
