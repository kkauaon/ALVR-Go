use alvr_common::prelude::*;
use alvr_session::AudioConfig;
use alvr_sockets::{StreamReceiver, StreamSender, AUDIO};
use oboe::{
    AudioInputCallback, AudioInputStreamSafe, AudioOutputCallback, AudioOutputStreamSafe,
    AudioStream, AudioStreamAsync, AudioStreamBuilder, DataCallbackResult, InputPreset, Mono,
    Output, PerformanceMode, SampleRateConversionQuality, Stereo, Usage,
};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    mem,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc as smpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::sync::mpsc as tmpsc;

struct RecorderCallback {
    sender: tmpsc::UnboundedSender<Vec<u8>>,
}

impl AudioInputCallback for RecorderCallback {
    type FrameType = (i16, Mono);

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioInputStreamSafe,
        frames: &[i16],
    ) -> DataCallbackResult {
        let mut sample_buffer = Vec::with_capacity(frames.len() * mem::size_of::<i16>());

        for frame in frames {
            sample_buffer.extend(&frame.to_ne_bytes());
        }

        self.sender.send(sample_buffer).ok();

        DataCallbackResult::Continue
    }
}

pub async fn record_audio_loop(sample_rate: u32, mut sender: StreamSender<()>) -> StrResult {
    let (_shutdown_notifier, shutdown_receiver) = smpsc::channel::<()>();
    let (data_sender, mut data_receiver) = tmpsc::unbounded_channel();

    thread::spawn(move || -> StrResult {
        let mut stream = trace_err!(AudioStreamBuilder::default()
            .set_shared()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sample_rate(sample_rate as _)
            .set_sample_rate_conversion_quality(SampleRateConversionQuality::Fastest)
            .set_mono()
            .set_i16()
            .set_input()
            .set_usage(Usage::VoiceCommunication)
            .set_input_preset(InputPreset::VoiceCommunication)
            .set_callback(RecorderCallback {
                sender: data_sender
            })
            .open_stream())?;

        trace_err!(stream.start())?;

        shutdown_receiver.recv().ok();

        // This call gets stuck if the headset goes to sleep, but finishes when the headset wakes up
        stream.stop_with_timeout(0).ok();

        Ok(())
    });

    while let Some(data) = data_receiver.recv().await {
        let mut buffer = sender.new_buffer(&(), data.len())?;
        buffer.get_mut().extend(data);
        sender.send_buffer(buffer).await.ok();
    }

    Ok(())
}

// The player stream can stop calling back without telling anybody: the Go only has OpenSL ES
// (Android 7), which reports neither disconnections nor errors, and the stream does not survive
// the headset going to sleep. The samples kept coming and only overflowed the buffer, and the game
// audio stayed silent until the app was restarted. So the stream is watched and reopened.
const PLAYER_STALL_TIMEOUT: Duration = Duration::from_secs(1);
// Longest wait between two attempts to reopen a stream that does not play (while the headset
// sleeps nothing can play, there is no point in retrying every second).
const PLAYER_MAX_RETRY_INTERVAL: Duration = Duration::from_secs(8);

// Shared between the audio callback and the thread that owns the stream. Atomics only: the
// callback is real-time.
struct PlayerActivity {
    start: Instant,
    last_callback_ms: AtomicU64,
    failed: AtomicBool,
}

impl PlayerActivity {
    fn now_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

struct PlayerCallback {
    sample_buffer: Arc<Mutex<VecDeque<f32>>>,
    activity: Arc<PlayerActivity>,
}

impl AudioOutputCallback for PlayerCallback {
    type FrameType = (f32, Stereo);

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioOutputStreamSafe,
        out_frames: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        // Fill exactly what is asked: the requested frames per callback is only a hint, and
        // indexing a smaller batch used to panic inside the callback, which killed the stream.
        let samples = alvr_audio::get_next_frame_batch(
            &mut *self.sample_buffer.lock(),
            2,
            out_frames.len(),
        );

        for (f, frame) in out_frames.iter_mut().enumerate() {
            *frame = (samples[f * 2], samples[f * 2 + 1]);
        }

        self.activity
            .last_callback_ms
            .store(self.activity.now_ms(), Ordering::Relaxed);

        DataCallbackResult::Continue
    }

    fn on_error_after_close(&mut self, _: &mut dyn AudioOutputStreamSafe, error: oboe::Error) {
        warn!("Game audio stream error: {error:?}");
        self.activity.failed.store(true, Ordering::Relaxed);
    }
}

fn open_player_stream(
    sample_rate: u32,
    batch_frames_count: usize,
    sample_buffer: &Arc<Mutex<VecDeque<f32>>>,
    activity: &Arc<PlayerActivity>,
) -> StrResult<AudioStreamAsync<Output, PlayerCallback>> {
    let mut stream = trace_err!(AudioStreamBuilder::default()
        .set_shared()
        .set_performance_mode(PerformanceMode::LowLatency)
        .set_sample_rate(sample_rate as _)
        .set_sample_rate_conversion_quality(SampleRateConversionQuality::Fastest)
        .set_stereo()
        .set_f32()
        .set_frames_per_callback(batch_frames_count as _)
        .set_output()
        .set_usage(Usage::Game)
        .set_callback(PlayerCallback {
            sample_buffer: Arc::clone(sample_buffer),
            activity: Arc::clone(activity),
        })
        .open_stream())?;

    trace_err!(stream.start())?;

    Ok(stream)
}

// Owns the player stream (it is !Send) until shutdown_receiver is signaled or dropped.
fn player_thread(
    sample_rate: u32,
    batch_frames_count: usize,
    sample_buffer: Arc<Mutex<VecDeque<f32>>>,
    shutdown_receiver: smpsc::Receiver<()>,
) {
    let activity = Arc::new(PlayerActivity {
        start: Instant::now(),
        last_callback_ms: AtomicU64::new(0),
        failed: AtomicBool::new(false),
    });

    let mut stream = None;
    let mut opened_at = Instant::now();
    let mut retry_interval = PLAYER_STALL_TIMEOUT;
    let mut reopen_count = 0;

    loop {
        match shutdown_receiver.recv_timeout(Duration::from_millis(250)) {
            Err(smpsc::RecvTimeoutError::Timeout) => (),
            _ => break,
        }

        if stream.is_none() {
            if opened_at.elapsed() < retry_interval && reopen_count > 0 {
                continue;
            }
            activity.failed.store(false, Ordering::Relaxed);
            activity
                .last_callback_ms
                .store(activity.now_ms(), Ordering::Relaxed);
            opened_at = Instant::now();
            match open_player_stream(sample_rate, batch_frames_count, &sample_buffer, &activity) {
                Ok(new_stream) => stream = Some(new_stream),
                Err(e) => {
                    warn!("Could not open the game audio stream: {e}");
                    reopen_count += 1;
                    retry_interval = (retry_interval * 2).min(PLAYER_MAX_RETRY_INTERVAL);
                }
            }
            continue;
        }

        let since_callback = Duration::from_millis(
            activity
                .now_ms()
                .saturating_sub(activity.last_callback_ms.load(Ordering::Relaxed)),
        );
        if activity.failed.load(Ordering::Relaxed) || since_callback > PLAYER_STALL_TIMEOUT {
            if let Some(mut old_stream) = stream.take() {
                // Note: Oboe crashes if stop() is not called before the stream is dropped
                old_stream.stop_with_timeout(0).ok();
            }
            reopen_count += 1;
            // back off while nothing plays (headset asleep), retry fast after a real playback
            retry_interval = if opened_at.elapsed() > PLAYER_MAX_RETRY_INTERVAL {
                PLAYER_STALL_TIMEOUT
            } else {
                (retry_interval * 2).min(PLAYER_MAX_RETRY_INTERVAL)
            };
            info!(
                "Game audio stopped playing ({} ms without data requests), reopening it",
                since_callback.as_millis()
            );
        }
    }

    if let Some(mut stream) = stream {
        // Note: Oboe crashes if stop() is not called before the stream is dropped
        stream.stop_with_timeout(0).ok();
    }
}

pub async fn play_audio_loop(
    sample_rate: u32,
    config: AudioConfig,
    receiver: StreamReceiver<()>,
) -> StrResult {
    let batch_frames_count = sample_rate as usize * config.batch_ms as usize / 1000;
    let average_buffer_frames_count =
        sample_rate as usize * config.average_buffering_ms as usize / 1000;

    let sample_buffer = Arc::new(Mutex::new(VecDeque::new()));

    // the stream lives in its own thread (it is !Send)
    let (_shutdown_notifier, shutdown_receiver) = smpsc::channel::<()>();
    thread::spawn({
        let sample_buffer = Arc::clone(&sample_buffer);
        move || player_thread(sample_rate, batch_frames_count, sample_buffer, shutdown_receiver)
    });

    alvr_audio::receive_samples_loop(
        receiver,
        sample_buffer,
        2,
        batch_frames_count,
        average_buffer_frames_count,
    )
    .await
}
