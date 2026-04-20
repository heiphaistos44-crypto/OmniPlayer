use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Horloge maître audio/vidéo — le clock audio est la référence principale.
/// La vidéo se synchronise dessus (drop/wait frames).
#[derive(Clone)]
pub struct MasterClock {
    inner: Arc<ClockInner>,
}

struct ClockInner {
    /// Position en microsecondes (×1_000_000).
    pos_us:    AtomicI64,
    /// Dernier instant réel où le clock a été mis à jour.
    updated:   parking_lot::Mutex<Instant>,
    paused:    AtomicI64,  // 0 = en cours, 1 = pausé
}

impl MasterClock {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ClockInner {
                pos_us:  AtomicI64::new(0),
                updated: parking_lot::Mutex::new(Instant::now()),
                paused:  AtomicI64::new(0),
            }),
        }
    }

    /// Met à jour le clock avec la position actuelle (en secondes, venant de l'audio).
    pub fn update(&self, pos_secs: f64) {
        let us = (pos_secs * 1_000_000.0) as i64;
        self.inner.pos_us.store(us, Ordering::Relaxed);
        *self.inner.updated.lock() = Instant::now();
    }

    /// Retourne la position estimée en secondes (interpolée depuis la dernière mise à jour).
    pub fn position_secs(&self) -> f64 {
        let base_us = self.inner.pos_us.load(Ordering::Relaxed) as f64;
        if self.is_paused() {
            return base_us / 1_000_000.0;
        }
        let elapsed = self.inner.updated.lock().elapsed().as_micros() as f64;
        (base_us + elapsed) / 1_000_000.0
    }

    pub fn seek(&self, pos_secs: f64) {
        self.update(pos_secs);
    }

    pub fn pause(&self) {
        self.inner.paused.store(1, Ordering::Relaxed);
        // Snapshot la position au moment de la pause
        let pos = self.position_secs();
        self.update(pos);
    }

    pub fn resume(&self) {
        self.inner.paused.store(0, Ordering::Relaxed);
        *self.inner.updated.lock() = Instant::now();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::Relaxed) != 0
    }
}

/// Décision de sync vidéo donnée par le clock.
#[derive(Debug, PartialEq)]
pub enum SyncDecision {
    /// Afficher cette frame maintenant.
    Present,
    /// Frame en retard → la dropper.
    Drop,
    /// Frame en avance → attendre `delay_ms` ms avant de l'afficher.
    Wait(u64),
}

/// Décision AV sync pour une frame vidéo avec ce PTS.
pub fn sync_decision(clock: &MasterClock, pts_secs: f64) -> SyncDecision {
    const SYNC_THRESHOLD: f64 = 0.010;  // 10 ms : tolérance
    const DROP_THRESHOLD: f64 = -0.050; // -50 ms : on droppe si trop en retard

    let diff = pts_secs - clock.position_secs();

    if diff < DROP_THRESHOLD {
        SyncDecision::Drop
    } else if diff.abs() <= SYNC_THRESHOLD {
        SyncDecision::Present
    } else if diff > 0.0 {
        SyncDecision::Wait((diff * 1000.0) as u64)
    } else {
        SyncDecision::Present
    }
}
