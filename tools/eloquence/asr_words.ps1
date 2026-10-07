<#
.SYNOPSIS
  Word timestamps for a speech recording via Windows SAPI dictation.

.DESCRIPTION
  These timings are the anchors align.py uses to map our segments onto the
  reference. Dictation drifts on long input, so the recording is first cut into
  chunks of at least MinChunkSeconds at pauses of at least MinGapSeconds; each
  chunk is recognized separately and word times are offset by the chunk start.
  A frame counts as pause when its 5 ms energy is SilenceDb below the
  95th-percentile frame energy.

  Output (tab-separated, no header): chunk, start_s, end_s, word, confidence.
  Input must be 16-bit PCM mono WAV. Recognition takes several minutes for a
  ten-minute recording.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File asr_words.ps1 reference.wav words.tsv
#>
param(
  [Parameter(Mandatory = $true)][string]$Reference,
  [Parameter(Mandatory = $true)][string]$Output,
  [double]$MinChunkSeconds = 2.5,
  [double]$MinGapSeconds = 0.04,
  [double]$SilenceDb = 27
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;

public static class WavChunker
{
    public static short[] ReadMono16(string path, out int sampleRate)
    {
        using (var reader = new BinaryReader(File.OpenRead(path)))
        {
            if (new string(reader.ReadChars(4)) != "RIFF") throw new InvalidDataException("not a RIFF file");
            reader.ReadInt32();
            if (new string(reader.ReadChars(4)) != "WAVE") throw new InvalidDataException("not a WAVE file");
            sampleRate = 0;
            while (reader.BaseStream.Position < reader.BaseStream.Length)
            {
                string id = new string(reader.ReadChars(4));
                int size = reader.ReadInt32();
                if (id == "fmt ")
                {
                    short format = reader.ReadInt16(), channels = reader.ReadInt16();
                    sampleRate = reader.ReadInt32();
                    reader.ReadInt32(); reader.ReadInt16();
                    short bits = reader.ReadInt16();
                    if (format != 1 || channels != 1 || bits != 16)
                        throw new InvalidDataException("expected 16-bit PCM mono");
                    reader.ReadBytes(size - 16);
                }
                else if (id == "data")
                {
                    byte[] bytes = reader.ReadBytes(size);
                    var samples = new short[bytes.Length / 2];
                    Buffer.BlockCopy(bytes, 0, samples, 0, samples.Length * 2);
                    return samples;
                }
                else reader.ReadBytes(size + (size & 1));
            }
            throw new InvalidDataException("no data chunk");
        }
    }

    // Returns [start, end) sample ranges, cut at the middle of qualifying pauses.
    public static List<int[]> Split(short[] x, int sampleRate, double minChunkS, double minGapS, double silenceDb)
    {
        int hop = (int)(0.005 * sampleRate), n = x.Length / hop;
        var db = new double[n];
        for (int f = 0; f < n; f++)
        {
            double sum = 0;
            for (int i = f * hop; i < (f + 1) * hop; i++) sum += (double)x[i] * x[i];
            db[f] = 10 * Math.Log10(sum / hop + 1e-9);
        }
        var sorted = (double[])db.Clone();
        Array.Sort(sorted);
        double threshold = sorted[(int)(0.95 * (n - 1))] - silenceDb;
        int minGap = (int)Math.Round(minGapS / 0.005), start = 0;
        var chunks = new List<int[]>();
        for (int i = 0; i < n; )
        {
            if (db[i] >= threshold) { i++; continue; }
            int j = i;
            while (j < n && db[j] < threshold) j++;
            if (j - i >= minGap && (i - start) * 0.005 >= minChunkS)
            {
                int cut = (i + j) / 2;
                chunks.Add(new[] { start * hop, cut * hop });
                start = cut;
            }
            i = j;
        }
        chunks.Add(new[] { start * hop, x.Length });
        return chunks;
    }
}
'@

$sampleRate = 0
$samples = [WavChunker]::ReadMono16((Resolve-Path $Reference).Path, [ref]$sampleRate)
$chunks = [WavChunker]::Split($samples, $sampleRate, $MinChunkSeconds, $MinGapSeconds, $SilenceDb)
$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo($sampleRate,
  [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
$lines = New-Object System.Collections.Generic.List[string]
for ($k = 0; $k -lt $chunks.Count; $k++) {
  $first, $end = $chunks[$k]
  $chunkStart = $first / $sampleRate
  $bytes = New-Object byte[] (2 * ($end - $first))
  [Buffer]::BlockCopy($samples, 2 * $first, $bytes, 0, $bytes.Length)
  $stream = New-Object System.IO.MemoryStream(, $bytes)
  $engine = New-Object System.Speech.Recognition.SpeechRecognitionEngine([Globalization.CultureInfo]'en-US')
  $engine.LoadGrammar((New-Object System.Speech.Recognition.DictationGrammar))
  $engine.SetInputToAudioStream($stream, $format)
  # Each Recognize() call reports positions relative to where it resumed, so
  # accumulate the consumed audio to get chunk-relative times.
  $offset = 0.0
  while ($true) {
    try { $result = $engine.Recognize() } catch { break }
    if ($null -eq $result) { break }
    $phraseStart = $result.Audio.AudioPosition.TotalSeconds + $offset
    foreach ($word in $result.Words) {
      $audio = $result.GetAudioForWordRange($word, $word)
      $t0 = $chunkStart + $phraseStart + ($audio.AudioPosition - $result.Audio.AudioPosition).TotalSeconds
      $lines.Add(("{0}`t{1:F3}`t{2:F3}`t{3}`t{4:F2}" -f $k, $t0, ($t0 + $audio.Duration.TotalSeconds), $word.Text, $word.Confidence))
    }
    $offset = $phraseStart + $result.Audio.Duration.TotalSeconds
  }
  $engine.Dispose()
  $stream.Dispose()
}
$outPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
[IO.File]::WriteAllLines($outPath, $lines)
Write-Host "$($chunks.Count) chunks, $($lines.Count) words -> $outPath"
