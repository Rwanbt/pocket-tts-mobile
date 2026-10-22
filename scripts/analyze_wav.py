#!/usr/bin/env python3
"""Analyse un WAV 16 bits mono : duree, RMS global, enveloppe RMS, silence, pic.

    python analyze_wav.py <wav> [...]
"""

import struct
import sys
import wave


def read_wav(path):
    with wave.open(path, "rb") as handle:
        channels = handle.getnchannels()
        width = handle.getsampwidth()
        rate = handle.getframerate()
        frames = handle.getnframes()
        raw = handle.readframes(frames)
    if width != 2:
        raise SystemExit(f"{path}: expected 16-bit samples, got {width * 8}-bit")
    count = len(raw) // 2
    samples = struct.unpack(f"<{count}h", raw[: count * 2])
    if channels > 1:
        samples = samples[::channels]
    return samples, rate


def report(path):
    samples, rate = read_wav(path)
    n = len(samples)
    duration = n / rate
    rms = (sum(s * s for s in samples) / n) ** 0.5 / 32768.0 if n else 0.0
    peak = max((abs(s) for s in samples), default=0) / 32768.0
    print(f"=== {path}")
    print(f"  samples={n} rate={rate}Hz duration={duration:.3f}s")
    print(f"  RMS={rms:.5f} ({20 * (rms ** 0 + 1e-12) ** 0:.0f}) peak={peak:.4f}")

    # Enveloppe RMS par fenetre de 50 ms.
    window = max(1, int(rate * 0.05))
    envelope = []
    for start in range(0, n, window):
        chunk = samples[start : start + window]
        if not chunk:
            break
        value = (sum(s * s for s in chunk) / len(chunk)) ** 0.5 / 32768.0
        envelope.append(value)
    if envelope:
        loud = max(envelope)
        silent = sum(1 for v in envelope if v < loud * 0.02)
        print(f"  frames_50ms={len(envelope)} loudest_window={loud:.5f}")
        print(f"  silence_ratio(<2% of peak window)={silent / len(envelope):.2%}")
        bars = "".join(
            " .:-=+*#%@"[min(9, int(v / loud * 9.999))] if loud > 0 else " " for v in envelope
        )
        print(f"  envelope: |{bars}|")


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        report(arg)
