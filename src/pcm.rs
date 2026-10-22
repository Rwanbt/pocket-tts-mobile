//! Contrat PCM partagé avec l'appelant Unifia.
//!
//! Unifia transporte l'audio comme octets IPC bruts :
//! `u32 LE fréquence || PCM16 LE mono`, un taux d'échantillonnage nul marquant
//! la fin de l'énoncé. Ce module reproduit exactement cet encodage pour que
//! les deux côtés ne puissent pas diverger, et pour que le format soit
//! vérifiable sans l'application.
//!
//! Le taux est celui du codec Mimi : 24 kHz mono.

/// Fréquence d'échantillonnage du codec Mimi.
pub const SAMPLE_RATE_HZ: u32 = 24_000;

/// Encode un chunk de samples en octets IPC.
///
/// Le format est `u32 LE fréquence || PCM16 LE mono` : les samples sont
/// écrêtés dans `[-1, 1]` puis convertis en PCM16 signé.
pub fn encode(sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + samples.len() * 2);
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// Encode un chunk au taux du codec Mimi.
pub fn encode_chunk(samples: &[f32]) -> Vec<u8> {
    encode(SAMPLE_RATE_HZ, samples)
}

/// Durée d'un chunk de samples, en secondes.
pub fn duration(samples: usize) -> f64 {
    samples as f64 / f64::from(SAMPLE_RATE_HZ)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Même vecteur que le test `chunk_is_rate_then_clamped_pcm16` d'Unifia :
    /// les deux implémentations doivent produire les mêmes octets.
    #[test]
    fn the_encoding_matches_the_unifia_vector() {
        let bytes = encode(24_000, &[0.5, -1.5, 1.0]);
        assert_eq!(&bytes[..4], &24_000u32.to_le_bytes());
        let pcm: Vec<i16> = bytes[4..]
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(pcm, vec![16_384, -32_767, 32_767]);
    }

    #[test]
    fn an_empty_chunk_still_carries_the_rate() {
        let bytes = encode_chunk(&[]);
        assert_eq!(bytes.len(), 4);
        assert_eq!(&bytes[..4], &24_000u32.to_le_bytes());
    }

    #[test]
    fn a_zero_rate_marks_the_end_of_the_utterance() {
        let bytes = encode(0, &[]);
        assert_eq!(bytes, 0u32.to_le_bytes().to_vec());
    }

    #[test]
    fn samples_are_clipped_not_wrapped() {
        // Sans écrêtage, 2.0 deborderait sur la valeur négative.
        let bytes = encode_chunk(&[2.0, -2.0]);
        let pcm: Vec<i16> = bytes[4..]
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(pcm, vec![i16::MAX, i16::MIN + 1]);
    }

    #[test]
    fn the_duration_follows_the_sample_rate() {
        assert!((duration(24_000) - 1.0).abs() < 1e-9);
        assert!((duration(12_000) - 0.5).abs() < 1e-9);
    }
}
