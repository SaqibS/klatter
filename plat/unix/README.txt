Klatter for Linux and FreeBSD
=============================

Klatter, the formant speech synthesizer, for Linux and FreeBSD: the klatter
command, and a Speech Dispatcher module so Orca and every program that speaks
through Speech Dispatcher can use it. Everything is in this folder.


WHICH PACKAGE

  x86_64       64-bit Intel and AMD PCs (most desktops and laptops)
  i686         32-bit Intel and AMD PCs
  aarch64      64-bit ARM: Raspberry Pi 4 and 5 with a 64-bit system, ARM laptops
  armv7        32-bit ARM: Raspberry Pi 2, 3 and 4 with a 32-bit system
  riscv64      64-bit RISC-V boards
  ppc64le      64-bit PowerPC: IBM POWER8 and newer, Raptor Talos and Blackbird
  s390x        IBM Z and LinuxONE mainframes
  loongarch64  Loongson 3A5000 and newer

  freebsd-x86_64, freebsd-aarch64 and freebsd-i686: FreeBSD 14 or newer.

Not sure? Run "uname -m": it prints the name to choose (arm64 is aarch64,
armv7l is armv7, i386 is i686, loong64 is loongarch64; on FreeBSD, amd64 is
x86_64).

The klatter program runs on any distribution (on IBM Z it needs glibc 2.17).
libklatter.so needs glibc 2.17 or newer (2.19 on PowerPC, 2.27 on RISC-V and
2.36 on LoongArch). On FreeBSD, "pkg install speech-dispatcher" to use it
with Orca; the klatter command plays through PulseAudio or PipeWire if
installed, else straight through /dev/dsp.


INSTALLING

  ./install.sh                 for you alone (into ~/.local; no password)
  sudo ./install.sh --system   for everyone on this computer

Then:   spd-say -o klatter "Hello from Klatter"
In Orca: Preferences, Voice, Speech synthesizer: klatter.
To take it out again: ./uninstall.sh (or sudo ./uninstall.sh --system).


THE KLATTER COMMAND

  klatter "Hello, world."                     speaks it
  echo "Some text" | klatter                  speaks standard input
  klatter -r 400 "Faster."                    words per minute, 30 to 1920
  klatter -p 130 -i 150 "Higher and livelier."  pitch in hertz, inflection in percent
  klatter --flutter 25 "A little waver."      flutter, off unless set
  klatter --set hs=115 --set br=20 "Hi."      any voice-quality setting
  klatter -s 44100 -w hello.wav "Saved."      a WAV file at any of the 20 rates
  klatter --list-settings                     every voice-quality setting
  klatter --help                              everything else

Your settings live in ~/.config/klatter/klatter.conf (install.sh puts a
commented copy there): the rate, pitch, flutter and every other setting,
which Speech Dispatcher and Orca then speak with.
