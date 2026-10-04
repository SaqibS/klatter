//! Minimal 16-bit PCM mono WAV writer.

use std::io::{self, Write};

pub fn write_wav<W: Write>(mut w: W, samples: &[i16], sample_rate: u32) -> io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_len).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&1u16.to_le_bytes())?; // mono
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&(sample_rate * 2).to_le_bytes())?;
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    w.write_all(&bytes)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    #[test]
    fn header_is_well_formed() {
        let mut buf = Vec::new();
        super::write_wav(&mut buf, &[0, 1, -1], 16000).unwrap();
        assert_eq!(buf.len(), 44 + 6);
        assert_eq!(&buf[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(buf[4..8].try_into().unwrap()), 36 + 6);
        assert_eq!(u32::from_le_bytes(buf[24..28].try_into().unwrap()), 16000);
        assert_eq!(&buf[36..40], b"data");
        assert_eq!(i16::from_le_bytes([buf[46], buf[47]]), 1);
    }
}
