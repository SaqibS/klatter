//! Phoneme inventory (ARPAbet) with features, Klatt (1979) inherent and minimum
//! durations, and synthesis targets based on Klatt (1980), Table III.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ph {
    IY,
    IH,
    EY,
    EH,
    AE,
    AA,
    AO,
    OW,
    UH,
    UW,
    AH,
    AX,
    IX,
    ER,
    AY,
    AW,
    OY,
    P,
    B,
    T,
    D,
    K,
    G,
    CH,
    JH,
    F,
    V,
    TH,
    DH,
    S,
    Z,
    SH,
    ZH,
    HH,
    M,
    N,
    NG,
    L,
    R,
    W,
    Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Vowel,
    Stop,
    Affricate,
    Fricative,
    Aspirate,
    Nasal,
    Liquid,
    Glide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Labial,
    Dental,
    Alveolar,
    PostAlveolar,
    Velar,
    Other,
}

/// Synthesis targets. For stops and affricates, the source amplitudes describe
/// the release burst or frication; closures are derived from `voiced`.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub f: [f64; 3],
    pub b: [f64; 3],
    /// Offglide target for diphthongs.
    pub f_end: Option<[f64; 3]>,
    pub av: f64,
    pub af: f64,
    pub ab: f64,
    /// Parallel amplitudes A2..A6.
    pub a: [f64; 5],
}

const fn sp(f: [f64; 3], b: [f64; 3], av: f64) -> Spec {
    Spec {
        f,
        b,
        f_end: None,
        av,
        af: 0.0,
        ab: 0.0,
        a: [0.0; 5],
    }
}

const fn dip(f: [f64; 3], b: [f64; 3], f_end: [f64; 3], av: f64) -> Spec {
    Spec {
        f,
        b,
        f_end: Some(f_end),
        av,
        af: 0.0,
        ab: 0.0,
        a: [0.0; 5],
    }
}

const fn fr(f: [f64; 3], b: [f64; 3], av: f64, af: f64, ab: f64, a: [f64; 5]) -> Spec {
    Spec {
        f,
        b,
        f_end: None,
        av,
        af,
        ab,
        a,
    }
}

impl Ph {
    pub const ALL: [Ph; 41] = [
        Ph::IY,
        Ph::IH,
        Ph::EY,
        Ph::EH,
        Ph::AE,
        Ph::AA,
        Ph::AO,
        Ph::OW,
        Ph::UH,
        Ph::UW,
        Ph::AH,
        Ph::AX,
        Ph::IX,
        Ph::ER,
        Ph::AY,
        Ph::AW,
        Ph::OY,
        Ph::P,
        Ph::B,
        Ph::T,
        Ph::D,
        Ph::K,
        Ph::G,
        Ph::CH,
        Ph::JH,
        Ph::F,
        Ph::V,
        Ph::TH,
        Ph::DH,
        Ph::S,
        Ph::Z,
        Ph::SH,
        Ph::ZH,
        Ph::HH,
        Ph::M,
        Ph::N,
        Ph::NG,
        Ph::L,
        Ph::R,
        Ph::W,
        Ph::Y,
    ];

    pub fn parse(s: &str) -> Option<Ph> {
        Ph::ALL.iter().copied().find(|p| p.name() == s)
    }

    pub fn name(self) -> &'static str {
        use Ph::*;
        match self {
            IY => "IY",
            IH => "IH",
            EY => "EY",
            EH => "EH",
            AE => "AE",
            AA => "AA",
            AO => "AO",
            OW => "OW",
            UH => "UH",
            UW => "UW",
            AH => "AH",
            AX => "AX",
            IX => "IX",
            ER => "ER",
            AY => "AY",
            AW => "AW",
            OY => "OY",
            P => "P",
            B => "B",
            T => "T",
            D => "D",
            K => "K",
            G => "G",
            CH => "CH",
            JH => "JH",
            F => "F",
            V => "V",
            TH => "TH",
            DH => "DH",
            S => "S",
            Z => "Z",
            SH => "SH",
            ZH => "ZH",
            HH => "HH",
            M => "M",
            N => "N",
            NG => "NG",
            L => "L",
            R => "R",
            W => "W",
            Y => "Y",
        }
    }

    pub fn class(self) -> Class {
        use Ph::*;
        match self {
            IY | IH | EY | EH | AE | AA | AO | OW | UH | UW | AH | AX | IX | ER | AY | AW | OY => {
                Class::Vowel
            }
            P | B | T | D | K | G => Class::Stop,
            CH | JH => Class::Affricate,
            F | V | TH | DH | S | Z | SH | ZH => Class::Fricative,
            HH => Class::Aspirate,
            M | N | NG => Class::Nasal,
            L | R => Class::Liquid,
            W | Y => Class::Glide,
        }
    }

    pub fn place(self) -> Place {
        use Ph::*;
        match self {
            P | B | F | V | M | W => Place::Labial,
            TH | DH => Place::Dental,
            T | D | S | Z | N | L => Place::Alveolar,
            CH | JH | SH | ZH | R | Y => Place::PostAlveolar,
            K | G | NG => Place::Velar,
            _ => Place::Other,
        }
    }

    pub fn is_vowel(self) -> bool {
        self.class() == Class::Vowel
    }

    pub fn is_sonorant(self) -> bool {
        matches!(
            self.class(),
            Class::Vowel | Class::Nasal | Class::Liquid | Class::Glide
        )
    }

    pub fn is_voiced(self) -> bool {
        use Ph::*;
        !matches!(self, P | T | K | CH | F | TH | S | SH | HH)
    }

    pub fn is_plosive(self) -> bool {
        self.class() == Class::Stop
    }

    /// Klatt (1979) inherent and minimum durations in ms.
    pub fn durations(self) -> (f64, f64) {
        use Ph::*;
        match self {
            AA => (240.0, 100.0),
            AE => (230.0, 80.0),
            AH => (140.0, 60.0),
            AO => (240.0, 100.0),
            AW => (260.0, 100.0),
            AX => (120.0, 60.0),
            AY => (250.0, 150.0),
            EH => (150.0, 70.0),
            ER => (180.0, 80.0),
            EY => (190.0, 70.0),
            IH => (135.0, 40.0),
            IX => (110.0, 40.0),
            IY => (155.0, 55.0),
            OW => (220.0, 80.0),
            OY => (280.0, 150.0),
            UH => (160.0, 60.0),
            UW => (210.0, 70.0),
            B => (85.0, 60.0),
            CH => (70.0, 50.0),
            D => (75.0, 50.0),
            DH => (50.0, 30.0),
            F => (100.0, 80.0),
            G => (80.0, 60.0),
            HH => (80.0, 20.0),
            JH => (70.0, 50.0),
            K => (80.0, 60.0),
            L => (80.0, 40.0),
            M => (70.0, 60.0),
            N => (60.0, 50.0),
            NG => (95.0, 60.0),
            P => (90.0, 50.0),
            R => (80.0, 30.0),
            S => (105.0, 60.0),
            SH => (105.0, 80.0),
            T => (75.0, 50.0),
            TH => (90.0, 60.0),
            V => (60.0, 40.0),
            W => (80.0, 60.0),
            Y => (80.0, 40.0),
            Z => (75.0, 40.0),
            ZH => (70.0, 40.0),
        }
    }

    pub fn spec(self) -> Spec {
        use Ph::*;
        match self {
            // Vowel AV levels match Eloquence's even loudness; a cascade alone
            // makes open vowels several dB louder than close ones. F1
            // bandwidths are wider than Klatt's so the F1 peak does not
            // dominate; Eloquence's vowels have more energy around it.
            // Formant targets are fitted to Eloquence's in running speech:
            // stressed vowels at their onsets and offsets, schwa across the
            // whole vowel because it is short and rarely reaches a target.
            IY => sp([250.0, 2100.0, 2790.0], [80.0, 200.0, 400.0], 62.0),
            IH => sp([330.0, 1750.0, 2530.0], [90.0, 100.0, 140.0], 62.0),
            EY => dip(
                [380.0, 1830.0, 2510.0],
                [120.0, 100.0, 200.0],
                [340.0, 1950.0, 2490.0],
                63.0,
            ),
            EH => sp([420.0, 1620.0, 2510.0], [105.0, 90.0, 200.0], 59.5),
            AE => sp([720.0, 1660.0, 2440.0], [120.0, 150.0, 320.0], 54.5),
            AA => sp([550.0, 1260.0, 2480.0], [230.0, 70.0, 160.0], 57.5),
            AO => sp([460.0, 1000.0, 2520.0], [160.0, 100.0, 80.0], 59.0),
            OW => dip(
                [390.0, 1070.0, 2420.0],
                [140.0, 70.0, 70.0],
                [360.0, 970.0, 2420.0],
                61.5,
            ),
            UH => sp([340.0, 1270.0, 2400.0], [140.0, 100.0, 80.0], 63.5),
            UW => dip(
                [240.0, 1400.0, 2510.0],
                [115.0, 110.0, 140.0],
                [260.0, 1290.0, 2510.0],
                63.0,
            ),
            AH => sp([410.0, 1320.0, 2530.0], [140.0, 50.0, 140.0], 60.5),
            AX => sp([380.0, 1345.0, 2510.0], [140.0, 90.0, 150.0], 61.0),
            IX => sp([420.0, 1650.0, 2400.0], [105.0, 90.0, 150.0], 61.0),
            ER => sp([390.0, 1280.0, 2000.0], [175.0, 60.0, 110.0], 59.5),
            AY => dip(
                [600.0, 1290.0, 2460.0],
                [175.0, 70.0, 200.0],
                [260.0, 2060.0, 2470.0],
                56.0,
            ),
            AW => dip(
                [660.0, 1500.0, 2520.0],
                [140.0, 70.0, 140.0],
                [310.0, 1100.0, 2340.0],
                55.5,
            ),
            OY => dip(
                [470.0, 930.0, 2440.0],
                [105.0, 50.0, 130.0],
                [280.0, 1790.0, 2490.0],
                60.0,
            ),

            W => sp([280.0, 860.0, 2270.0], [50.0, 80.0, 60.0], 57.0),
            Y => sp([270.0, 1980.0, 2880.0], [40.0, 250.0, 500.0], 57.0),
            R => sp([370.0, 1260.0, 1940.0], [70.0, 100.0, 120.0], 57.0),
            L => sp([300.0, 1030.0, 3000.0], [50.0, 100.0, 280.0], 57.0),
            // Nasal F2 is fitted to Eloquence's nasal murmur.
            M => sp([480.0, 1220.0, 2270.0], [40.0, 200.0, 200.0], 57.0),
            N => sp([480.0, 1550.0, 2590.0], [40.0, 300.0, 300.0], 57.0),
            NG => sp([480.0, 1870.0, 2660.0], [160.0, 150.0, 400.0], 57.0),

            F => fr(
                [340.0, 1100.0, 2080.0],
                [200.0, 120.0, 150.0],
                0.0,
                60.0,
                41.0,
                [0.0; 5],
            ),
            V => fr(
                [220.0, 1100.0, 2080.0],
                [60.0, 90.0, 120.0],
                47.0,
                55.0,
                47.0,
                [0.0; 5],
            ),
            TH => fr(
                [320.0, 1290.0, 2540.0],
                [200.0, 90.0, 200.0],
                0.0,
                60.0,
                43.0,
                [0.0, 0.0, 0.0, 0.0, 22.0],
            ),
            DH => fr(
                [270.0, 1290.0, 2540.0],
                [60.0, 80.0, 170.0],
                47.0,
                55.0,
                50.0,
                [0.0, 0.0, 0.0, 0.0, 40.0],
            ),
            S => fr(
                [320.0, 1390.0, 2530.0],
                [200.0, 80.0, 200.0],
                0.0,
                60.0,
                0.0,
                [0.0, 0.0, 0.0, 48.0, 54.0],
            ),
            Z => fr(
                [240.0, 1390.0, 2530.0],
                [70.0, 60.0, 180.0],
                47.0,
                55.0,
                0.0,
                [0.0, 0.0, 0.0, 47.0, 51.0],
            ),
            SH => fr(
                [300.0, 1840.0, 2750.0],
                [200.0, 100.0, 300.0],
                0.0,
                60.0,
                0.0,
                [0.0, 63.0, 55.0, 47.0, 42.0],
            ),
            ZH => fr(
                [300.0, 1840.0, 2750.0],
                [60.0, 100.0, 300.0],
                47.0,
                55.0,
                0.0,
                [0.0, 58.0, 50.0, 48.0, 45.0],
            ),
            HH => sp([500.0, 1500.0, 2500.0], [300.0, 150.0, 200.0], 0.0),

            P => fr(
                [400.0, 1100.0, 2150.0],
                [300.0, 150.0, 220.0],
                0.0,
                60.0,
                55.0,
                [0.0; 5],
            ),
            B => fr(
                [200.0, 1100.0, 2150.0],
                [60.0, 110.0, 130.0],
                0.0,
                57.0,
                50.0,
                [0.0; 5],
            ),
            T => fr(
                [400.0, 1600.0, 2600.0],
                [300.0, 120.0, 250.0],
                0.0,
                60.0,
                0.0,
                [0.0, 25.0, 40.0, 47.0, 55.0],
            ),
            D => fr(
                [200.0, 1600.0, 2600.0],
                [60.0, 100.0, 170.0],
                0.0,
                57.0,
                0.0,
                [0.0, 20.0, 35.0, 42.0, 50.0],
            ),
            K => fr(
                [300.0, 1990.0, 2850.0],
                [250.0, 160.0, 330.0],
                0.0,
                60.0,
                0.0,
                [47.0, 44.0, 35.0, 35.0, 0.0],
            ),
            G => fr(
                [200.0, 1990.0, 2850.0],
                [60.0, 150.0, 280.0],
                0.0,
                57.0,
                0.0,
                [42.0, 39.0, 30.0, 30.0, 0.0],
            ),
            CH => fr(
                [350.0, 1800.0, 2820.0],
                [200.0, 90.0, 300.0],
                0.0,
                60.0,
                0.0,
                [0.0, 63.0, 55.0, 53.0, 50.0],
            ),
            JH => fr(
                [260.0, 1800.0, 2820.0],
                [60.0, 80.0, 270.0],
                47.0,
                55.0,
                0.0,
                [0.0, 58.0, 50.0, 48.0, 45.0],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for p in Ph::ALL {
            assert_eq!(Ph::parse(p.name()), Some(p));
        }
    }

    #[test]
    fn minimum_never_exceeds_inherent() {
        for p in Ph::ALL {
            let (inh, min) = p.durations();
            assert!(min <= inh, "{p:?}");
        }
    }
}
