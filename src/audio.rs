use rodio::Source;
use spectrum_analyzer::scaling::divide_by_N;
use spectrum_analyzer::{FrequencyLimit, samples_fft_to_spectrum};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

// Audio Analyzer Wrapper
pub struct AudioAnalyzer<I>
where
    I: Iterator<Item = f32> + Source<Item = f32>,
{
    pub input: I,
    pub buffer: Vec<f32>,
    pub spectrum_data: Arc<Mutex<Vec<(&'static str, u64)>>>,
    pub sample_rate: u32,
}

impl<I> Iterator for AudioAnalyzer<I>
where
    I: Iterator<Item = f32> + Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next();
        if let Some(s) = sample {
            self.buffer.push(s);

            // Analyze every 2048 samples (approx 46ms at 44.1kHz)
            if self.buffer.len() >= 2048 {
                let spectrum = samples_fft_to_spectrum(
                    &self.buffer,
                    self.sample_rate,
                    FrequencyLimit::Range(20.0, 20_000.0),
                    Some(&divide_by_N),
                );

                if let Ok(spec) = spectrum {
                    // Map spectrum to 8 bars
                    let mut bars = [0u64; 8];
                    for (freq, val) in spec.data() {
                        let freq_val = freq.val();
                        let idx = if freq_val < 100.0 {
                            0
                        } else if freq_val < 250.0 {
                            1
                        } else if freq_val < 500.0 {
                            2
                        } else if freq_val < 1000.0 {
                            3
                        } else if freq_val < 2000.0 {
                            4
                        } else if freq_val < 4000.0 {
                            5
                        } else if freq_val < 8000.0 {
                            6
                        } else {
                            7
                        };

                        // Scale value (logarithmic-ish)
                        let height = (val.val() * 1000.0) as u64;
                        if height > bars[idx] {
                            bars[idx] = height;
                        }
                    }

                    // Update shared state
                    if let Ok(mut data) = self.spectrum_data.lock() {
                        if data.len() == 8 {
                            data[0].1 = bars[0].clamp(0, 100);
                            data[1].1 = bars[1].clamp(0, 100);
                            data[2].1 = bars[2].clamp(0, 100);
                            data[3].1 = bars[3].clamp(0, 100);
                            data[4].1 = bars[4].clamp(0, 100);
                            data[5].1 = bars[5].clamp(0, 100);
                            data[6].1 = bars[6].clamp(0, 100);
                            data[7].1 = bars[7].clamp(0, 100);
                        } else {
                            *data = vec![
                                ("Sub", bars[0].clamp(0, 100)),
                                ("Bass", bars[1].clamp(0, 100)),
                                ("LowM", bars[2].clamp(0, 100)),
                                ("Mid", bars[3].clamp(0, 100)),
                                ("HighM", bars[4].clamp(0, 100)),
                                ("Pres", bars[5].clamp(0, 100)),
                                ("Bril", bars[6].clamp(0, 100)),
                                ("Air", bars[7].clamp(0, 100)),
                            ];
                        }
                    }
                }
                self.buffer.clear();
            }
        }
        sample
    }
}

impl<I> Source for AudioAnalyzer<I>
where
    I: Iterator<Item = f32> + Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.input.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let res = self.input.try_seek(pos);
        if res.is_ok() {
            self.buffer.clear();
        }
        res
    }
}

#[cfg(test)]
mod tests;
