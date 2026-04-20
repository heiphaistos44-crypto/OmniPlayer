use anyhow::Result;
use omni_core::pipeline::{MediaPipeline, PipelineCommand, PipelineEvent};
use omni_core::probe::MediaInfo;
use parking_lot::Mutex;
use std::sync::Arc;

/// État de la machine à états du lecteur.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Buffering(u8),
    EndOfFile,
    Error(String),
}

/// Interface principale du lecteur : reçoit les commandes UI, orchestre le pipeline.
pub struct Player {
    pub state:        PlayerState,
    pub duration:     f64,
    pub position:     f64,
    pub volume:       f32,
    pub media_info:   Option<MediaInfo>,
    pipeline:         Option<MediaPipeline>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            state:      PlayerState::Idle,
            duration:   0.0,
            position:   0.0,
            volume:     1.0,
            media_info: None,
            pipeline:   None,
        }
    }

    /// Ouvre et démarre la lecture d'un fichier ou URL.
    pub fn open(&mut self, path: &str) -> Result<()> {
        // Arrête l'ancienne lecture
        if let Some(p) = &self.pipeline {
            p.send_command(PipelineCommand::Stop);
        }

        self.state      = PlayerState::Loading;
        self.duration   = 0.0;
        self.position   = 0.0;
        self.media_info = None;

        let pipeline = MediaPipeline::launch(path.to_string())?;
        self.pipeline = Some(pipeline);
        Ok(())
    }

    pub fn play_pause(&mut self) {
        match &self.state {
            PlayerState::Playing => {
                if let Some(p) = &self.pipeline {
                    p.send_command(PipelineCommand::Pause);
                }
                self.state = PlayerState::Paused;
            }
            PlayerState::Paused => {
                if let Some(p) = &self.pipeline {
                    p.send_command(PipelineCommand::Resume);
                }
                self.state = PlayerState::Playing;
            }
            _ => {}
        }
    }

    pub fn seek(&mut self, pos: f64) {
        if let Some(p) = &self.pipeline {
            p.send_command(PipelineCommand::Seek(pos));
            self.position = pos;
        }
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v;
        if let Some(p) = &self.pipeline {
            p.send_command(PipelineCommand::SetVolume(v));
        }
    }

    pub fn stop(&mut self) {
        if let Some(p) = &self.pipeline {
            p.send_command(PipelineCommand::Stop);
        }
        self.pipeline = None;
        self.state    = PlayerState::Idle;
        self.position = 0.0;
    }

    /// À appeler chaque frame UI — traite les événements pipeline.
    pub fn poll_events(&mut self) {
        let pipeline = match &self.pipeline { Some(p) => p, None => return };

        while let Some(event) = pipeline.try_recv_event() {
            match event {
                PipelineEvent::DurationKnown(d)    => { self.duration = d; }
                PipelineEvent::PositionChanged(p)   => {
                    self.position = p;
                    if self.state == PlayerState::Loading {
                        self.state = PlayerState::Playing;
                    }
                }
                PipelineEvent::BufferingProgress(b) => {
                    self.state = PlayerState::Buffering(b);
                }
                PipelineEvent::EndOfStream => {
                    self.state = PlayerState::EndOfFile;
                }
                PipelineEvent::Error(e) => {
                    self.state = PlayerState::Error(e);
                }
                PipelineEvent::MetadataReady(info) => {
                    self.media_info = Some(*info);
                }
            }
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, PlayerState::Playing | PlayerState::Paused | PlayerState::Buffering(_))
    }
}
