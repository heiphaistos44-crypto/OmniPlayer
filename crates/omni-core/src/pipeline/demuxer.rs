use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use ffmpeg_next as ffmpeg;

use crate::decoder::{
    audio::AudioDecoder, video::VideoDecoder, DecodedAudioFrame, DecodedVideoFrame,
};
use crate::decoder::context::DecodeContext;
use crate::pipeline::{PipelineCommand, PipelineEvent};
use crate::probe;

/// Boucle principale du thread demuxer.
/// Lit les paquets, les distribue aux décodeurs, envoie les frames aux queues.
pub fn run_demuxer(
    path:     &str,
    cmd_rx:   Receiver<PipelineCommand>,
    event_tx: Sender<PipelineEvent>,
    video_tx: Sender<DecodedVideoFrame>,
    audio_tx: Sender<DecodedAudioFrame>,
) -> Result<()> {
    // Sonde d'abord les métadonnées
    let info = probe::probe_file(std::path::Path::new(path))
        .unwrap_or_else(|_| probe::MediaInfo {
            path: path.to_string(),
            duration_secs: 0.0,
            video: None,
            audio: vec![],
            subtitles: vec![],
            chapters: vec![],
            format_name: "unknown".into(),
            bit_rate: 0,
        });

    let duration = info.duration_secs;
    let _ = event_tx.send(PipelineEvent::MetadataReady(Box::new(info)));
    let _ = event_tx.send(PipelineEvent::DurationKnown(duration));

    let mut ctx = DecodeContext::open(path, Some("dxva2"))?;

    let v_idx = ctx.video_stream_idx;
    let a_idx = ctx.audio_stream_idx;

    // Time bases des streams
    let v_tb = v_idx.map(|i| {
        let s = ctx.format_ctx.stream(i).unwrap();
        s.time_base().numerator() as f64 / s.time_base().denominator() as f64
    }).unwrap_or(0.0);
    let a_tb = a_idx.map(|i| {
        let s = ctx.format_ctx.stream(i).unwrap();
        s.time_base().numerator() as f64 / s.time_base().denominator() as f64
    }).unwrap_or(0.0);

    let mut video_dec = v_idx
        .map(|_| ctx.build_video_decoder().map(|d| VideoDecoder::new(d, v_tb)))
        .transpose()?
        .and_then(|r| r.ok());

    let mut audio_dec = a_idx
        .map(|_| ctx.build_audio_decoder().map(|d| AudioDecoder::new(d, a_tb)))
        .transpose()?
        .and_then(|r| r.ok());

    let mut paused = false;

    'main: loop {
        // Traite les commandes en attente
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                PipelineCommand::Stop => break 'main,
                PipelineCommand::Pause  => { paused = true; }
                PipelineCommand::Resume => { paused = false; }
                PipelineCommand::Seek(pos) => {
                    ctx.seek(pos)?;
                    // Vide les queues après seek (non-blocking)
                    while video_tx.try_recv().is_ok() {}
                    while audio_tx.try_recv().is_ok() {}
                }
                _ => {}
            }
        }

        if paused {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        // Backpressure: si les queues sont pleines, on attend
        if video_tx.is_full() || audio_tx.is_full() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }

        // Lit le prochain paquet
        let mut packet = ffmpeg::Packet::empty();
        match ctx.format_ctx.read((&mut packet).into()) {
            Ok(_) => {}
            Err(ffmpeg::Error::Eof) => {
                // Vide les décodeurs
                flush_decoders(&mut video_dec, &mut audio_dec, &video_tx, &audio_tx);
                let _ = event_tx.send(PipelineEvent::EndOfStream);
                break 'main;
            }
            Err(e) => {
                let _ = event_tx.send(PipelineEvent::Error(e.to_string()));
                break 'main;
            }
        }

        // Distribue le paquet au bon décodeur
        let stream_idx = packet.stream();

        if Some(stream_idx) == v_idx {
            if let Some(dec) = &mut video_dec {
                let _ = dec.send_packet(&packet);
                while let Ok(Some(frame)) = dec.receive_frame() {
                    let _ = video_tx.try_send(frame);
                }
            }
        } else if Some(stream_idx) == a_idx {
            if let Some(dec) = &mut audio_dec {
                let _ = dec.send_packet(&packet);
                while let Ok(Some(frame)) = dec.receive_frame() {
                    // Mise à jour position via audio
                    let pos = frame.pts_secs;
                    let _ = event_tx.try_send(PipelineEvent::PositionChanged(pos));
                    let _ = audio_tx.try_send(frame);
                }
            }
        }
    }

    Ok(())
}

fn flush_decoders(
    video_dec: &mut Option<VideoDecoder>,
    audio_dec: &mut Option<AudioDecoder>,
    video_tx:  &Sender<DecodedVideoFrame>,
    audio_tx:  &Sender<DecodedAudioFrame>,
) {
    if let Some(dec) = video_dec {
        let _ = dec.send_eof();
        while let Ok(Some(f)) = dec.receive_frame() {
            let _ = video_tx.try_send(f);
        }
    }
    if let Some(dec) = audio_dec {
        let _ = dec.send_eof();
        while let Ok(Some(f)) = dec.receive_frame() {
            let _ = audio_tx.try_send(f);
        }
    }
}
