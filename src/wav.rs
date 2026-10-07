//! Minimal 16-bit PCM mono WAV writer.

use std::io::{self, Write};

fn sizes(sample_count: usize, sample_rate: u32) -> io::Result<(u32, u32, u32)> {
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "audio exceeds PCM WAV size limits",
        )
    };
    let data_len = u32::try_from(sample_count)
        .ok()
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(invalid)?;
    let riff_len = data_len.checked_add(36).ok_or_else(invalid)?;
    let byte_rate = sample_rate.checked_mul(2).ok_or_else(invalid)?;
    Ok((data_len, riff_len, byte_rate))
}

pub fn write_wav<W: Write>(mut w: W, samples: &[i16], sample_rate: u32) -> io::Result<()> {
    let (data_len, riff_len, byte_rate) = sizes(samples.len(), sample_rate)?;
    w.write_all(b"RIFF")?;
    w.write_all(&riff_len.to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&1u16.to_le_bytes())?; // mono
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&byte_rate.to_le_bytes())?;
    w.write_all(&2u16.to_le_bytes())?;
    w.write_all(&16u16.to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_len.to_le_bytes())?;
    let mut bytes = [0u8; 8192];
    for samples in samples.chunks(bytes.len() / 2) {
        for (s, out) in samples.iter().zip(bytes.as_chunks_mut::<2>().0) {
            out.copy_from_slice(&s.to_le_bytes());
        }
        w.write_all(&bytes[..samples.len() * 2])?;
    }
    w.flush()
}

#[cfg(test)]
mod tests {
    #[test]
    fn block_writes_preserve_all_samples() {
        let samples: Vec<i16> = (0..10001).map(|n| (n * 31) as i16).collect();
        let mut output = Vec::new();
        super::write_wav(&mut output, &samples, 16000).unwrap();
        let expected: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        assert_eq!(&output[44..], expected);
    }

    #[test]
    fn riff_sizes_are_checked_before_writing() {
        let max_samples = ((u32::MAX - 36) / 2) as usize;
        assert!(super::sizes(max_samples, 16000).is_ok());
        for (count, rate) in [(max_samples + 1, 16000), (usize::MAX, 16000), (1, u32::MAX)] {
            assert_eq!(
                super::sizes(count, rate).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

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
