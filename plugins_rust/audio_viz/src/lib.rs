use pyo3::prelude::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use rustfft::{FftPlanner, num_complex::Complex};
use std::sync::{Arc, Mutex};

// ── Глобальний стрім (живе поки UI працює) ────────────────────────────────────
const FFT_SIZE: usize = 1024;

struct AudioState {
    samples: Vec<f32>,
    volume: f32,
}

impl AudioState {
    fn new() -> Self {
        Self {
            samples: vec![0.0; FFT_SIZE],
            volume: 0.0,
        }
    }
}

struct GlobalStream {
    _stream: Stream,
    state: Arc<Mutex<AudioState>>,
}

// SAFETY: Stream містить raw pointer, але ми керуємо ним через Arc<Mutex>
unsafe impl Send for GlobalStream {}

static STREAM_HOLDER: Mutex<Option<GlobalStream>> = Mutex::new(None);

// ── Оновлення буфера семплів ──────────────────────────────────────────────────
fn update_state(state: &Arc<Mutex<AudioState>>, chunk: &[f32]) {
    if let Ok(mut st) = state.lock() {
        let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
        st.volume = rms;
        let buf = &mut st.samples;
        let n = chunk.len().min(FFT_SIZE);
        if n > 0 {
            buf.drain(..n.min(buf.len()));
            buf.extend_from_slice(&chunk[..n]);
            if buf.len() > FFT_SIZE {
                let excess = buf.len() - FFT_SIZE;
                buf.drain(..excess);
            }
        }
    }
}

// ── Конвертація форматів у f32 ────────────────────────────────────────────────
fn i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter().map(|&s| s as f32 / i16::MAX as f32).collect()
}

fn u16_to_f32(data: &[u16]) -> Vec<f32> {
    data.iter()
        .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
        .collect()
}

// ── Побудова стріму ───────────────────────────────────────────────────────────
fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    state: Arc<Mutex<AudioState>>,
) -> Result<Stream, cpal::BuildStreamError> {
    let err_fn = |e| eprintln!("[audio_viz] stream error: {e}");
    match config.sample_format() {
        SampleFormat::F32 => {
            let s = Arc::clone(&state);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[f32], _| update_state(&s, data),
                err_fn,
                None,
            )
        }
        SampleFormat::I16 => {
            let s = Arc::clone(&state);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[i16], _| update_state(&s, &i16_to_f32(data)),
                err_fn,
                None,
            )
        }
        SampleFormat::U16 => {
            let s = Arc::clone(&state);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[u16], _| update_state(&s, &u16_to_f32(data)),
                err_fn,
                None,
            )
        }
        _ => {
            // Fallback: спробуємо як f32
            let s = Arc::clone(&state);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[f32], _| update_state(&s, data),
                err_fn,
                None,
            )
        }
    }
}

#[pymodule]
mod audio_viz {
    use super::*;

    /// Запустити постійний аудіо стрім.
    /// Викликати один раз при старті UI.
    #[pyfunction]
    fn start_stream() -> PyResult<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No input device"))?;
        let config = device
            .default_input_config()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let state = Arc::new(Mutex::new(AudioState::new()));
        let stream = build_stream(&device, &config, Arc::clone(&state))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        stream
            .play()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let mut holder = STREAM_HOLDER
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
        *holder = Some(GlobalStream { _stream: stream, state });
        Ok(())
    }

    /// Зупинити стрім.
    #[pyfunction]
    fn stop_stream() -> PyResult<()> {
        let mut holder = STREAM_HOLDER
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
        *holder = None;
        Ok(())
    }

    /// Поточна RMS гучність (0.0–1.0).
    /// Якщо стрім не запущено — одноразовий замір (фолбек).
    #[pyfunction]
    fn get_volume() -> PyResult<f32> {
        let holder = STREAM_HOLDER
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
        if let Some(gs) = holder.as_ref() {
            let st = gs.state.lock()
                .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("State poisoned"))?;
            return Ok(st.volume);
        }
        drop(holder);
        one_shot_volume()
    }

    /// FFT-смуги: повертає список із `n_bands` значень (0.0–1.0).
    /// Логарифмічна шкала — виглядає як еквалайзер.
    /// Потребує start_stream().
    #[pyfunction]
    fn get_frequency_bands(n_bands: usize) -> PyResult<Vec<f32>> {
        if n_bands == 0 {
            return Ok(vec![]);
        }

        let samples = {
            let holder = STREAM_HOLDER
                .lock()
                .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
            match holder.as_ref() {
                Some(gs) => {
                    let st = gs.state.lock()
                        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("State poisoned"))?;
                    st.samples.clone()
                }
                None => vec![0.0f32; FFT_SIZE],
            }
        };

        // Вікно Ганна — зменшує спектральні витоки
        let mut buffer: Vec<Complex<f32>> = samples
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32
                    / (FFT_SIZE as f32 - 1.0)).cos());
                Complex { re: s * w, im: 0.0 }
            })
            .collect();

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        fft.process(&mut buffer);

        // Лише перша половина спектру (дзеркальна симетрія)
        let half = FFT_SIZE / 2;
        let magnitudes: Vec<f32> = buffer[..half]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt())
            .collect();

        // Логарифмічні смуги: перші смуги — низькі частоти (більше деталей)
        let mut bands = vec![0.0f32; n_bands];
        for (i, band) in bands.iter_mut().enumerate() {
            let t0 = (i as f32 / n_bands as f32).powf(1.5);
            let t1 = ((i + 1) as f32 / n_bands as f32).powf(1.5);
            let start = (half as f32 * t0) as usize;
            let end = ((half as f32 * t1) as usize).max(start + 1).min(half);
            let slice = &magnitudes[start..end];
            if !slice.is_empty() {
                *band = slice.iter().cloned().fold(0.0f32, f32::max);
            }
        }

        // Нормалізація до 0.0–1.0
        let max_val = bands.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
        Ok(bands.iter().map(|&v| (v / max_val).clamp(0.0, 1.0)).collect())
    }

    /// Список доступних мікрофонів.
    #[pyfunction]
    fn get_input_devices() -> PyResult<Vec<String>> {
        let host = cpal::default_host();
        let devices = host
            .input_devices()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(devices.filter_map(|d| d.name().ok()).collect())
    }
}

// ── Фолбек: одноразовий замір без стріму ─────────────────────────────────────
fn one_shot_volume() -> PyResult<f32> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No input device"))?;
    let config = device
        .default_input_config()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    let volume = Arc::new(Mutex::new(0.0f32));
    let v = Arc::clone(&volume);

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let rms = (data.iter().map(|s| s * s).sum::<f32>() / data.len() as f32).sqrt();
                *v.lock().unwrap() = rms;
            },
            |e| eprintln!("[audio_viz] one_shot error: {e}"),
            None,
        ),
        _ => return Err(pyo3::exceptions::PyRuntimeError::new_err("Unsupported format in fallback")),
    }
    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

    stream.play()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    std::thread::sleep(std::time::Duration::from_millis(80));
    let v = *volume.lock().unwrap();
    Ok(v)
}
