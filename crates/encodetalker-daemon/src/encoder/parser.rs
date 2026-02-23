use encodetalker_common::EncodingStats;
use regex::Regex;
use std::time::Duration;

static FRAME_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"frame=\s*(\d+)").unwrap());
static FPS_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"fps=\s*([\d.]+)").unwrap());
static BITRATE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"bitrate=\s*([\d.]+)").unwrap());
static TIME_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"time=(\d{2}):(\d{2}):(\d{2})\.(\d{2})").unwrap());
// Format SvtAv1EncApp avec --progress 2: "Encoding frame   3456 1234.56 kbps 210.12 fps"
// Note: espaces multiples entre "frame" et le numéro
static ENCODER_REGEX: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"Encoding\s+frame\s+(\d+)\s+([\d.]+)\s+kbps\s+([\d.]+)\s+fps").unwrap()
});
// Format aomenc: "Pass 1/2 frame  268/229    54960B   14288 ms 18.76 fps [ETA  unknown]"
static AOMENC_REGEX: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"Pass\s+(\d)/(\d)\s+frame\s+(\d+)/\d+\s+\d+B\s+\d+\s+ms\s+([\d.]+)\s+fps").unwrap()
});

/// Parser de stats `FFmpeg` depuis stderr
pub struct StatsParser {
    stats: EncodingStats,
}

impl StatsParser {
    #[must_use]
    pub fn new(total_frames: Option<u64>, total_duration: Option<Duration>) -> Self {
        let stats = EncodingStats {
            total_frames,
            total_duration,
            ..Default::default()
        };
        Self { stats }
    }

    /// Parser une ligne de sortie ffmpeg
    pub fn parse_line(&mut self, line: &str) {
        // Format -progress : key=value sur des lignes séparées
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "frame" => {
                    if let Ok(frame) = value.parse::<u64>() {
                        self.stats.frame = frame;
                    }
                }
                "fps" => {
                    if let Ok(fps) = value.parse::<f64>() {
                        self.stats.fps = fps;
                    }
                }
                "bitrate" => {
                    // Format: "1234.5kbits/s"
                    let bitrate_str = value.trim_end_matches("kbits/s");
                    if let Ok(bitrate) = bitrate_str.parse::<f64>() {
                        self.stats.bitrate = bitrate;
                    }
                }
                "out_time" => {
                    // Format: "00:00:05.000000"
                    if let Some(caps) = TIME_REGEX.captures(value) {
                        if let (Ok(hours), Ok(mins), Ok(secs), Ok(centis)) = (
                            caps[1].parse::<u64>(),
                            caps[2].parse::<u64>(),
                            caps[3].parse::<u64>(),
                            caps[4].parse::<u64>(),
                        ) {
                            let total_secs = hours * 3600 + mins * 60 + secs;
                            let total_millis = total_secs * 1000 + centis * 10;
                            self.stats.time_encoded = Duration::from_millis(total_millis);
                        }
                    }
                }
                "progress" => {
                    // Recalculer progression et ETA quand on reçoit progress=continue
                    if value == "continue" || value == "end" {
                        self.stats.update();
                    }
                }
                _ => {}
            }
        } else {
            // Fallback : format classique pour compatibilité
            // Parse frame
            if let Some(caps) = FRAME_REGEX.captures(line) {
                if let Ok(frame) = caps[1].parse::<u64>() {
                    self.stats.frame = frame;
                }
            }

            // Parse fps
            if let Some(caps) = FPS_REGEX.captures(line) {
                if let Ok(fps) = caps[1].parse::<f64>() {
                    self.stats.fps = fps;
                }
            }

            // Parse bitrate
            if let Some(caps) = BITRATE_REGEX.captures(line) {
                if let Ok(bitrate) = caps[1].parse::<f64>() {
                    self.stats.bitrate = bitrate;
                }
            }

            // Parse time (format: 00:00:49.40)
            if let Some(caps) = TIME_REGEX.captures(line) {
                if let (Ok(hours), Ok(mins), Ok(secs), Ok(centis)) = (
                    caps[1].parse::<u64>(),
                    caps[2].parse::<u64>(),
                    caps[3].parse::<u64>(),
                    caps[4].parse::<u64>(),
                ) {
                    let total_secs = hours * 3600 + mins * 60 + secs;
                    let total_millis = total_secs * 1000 + centis * 10;
                    self.stats.time_encoded = Duration::from_millis(total_millis);
                }
            }

            // Recalculer progression et ETA
            self.stats.update();
        }
    }

    /// Obtenir les stats actuelles
    #[must_use]
    pub fn get_stats(&self) -> &EncodingStats {
        &self.stats
    }

    /// Obtenir une copie des stats
    #[must_use]
    pub fn clone_stats(&self) -> EncodingStats {
        self.stats.clone()
    }

    /// Parser une ligne de sortie `SvtAv1EncApp` ou aomenc
    /// Format `SvtAv1EncApp`: "Encoding frame   3456 1234.56 kbps 210.12 fps"
    /// Format aomenc: "Pass 1/2 frame  268/229    54960B   14288 ms 18.76 fps [ETA  unknown]"
    pub fn parse_encoder_line(&mut self, line: &str) {
        // Essayer le format SVT-AV1 d'abord
        if let Some(caps) = ENCODER_REGEX.captures(line) {
            if let Ok(frame) = caps[1].parse::<u64>() {
                self.stats.frame = frame;
            }
            if let Ok(bitrate) = caps[2].parse::<f64>() {
                self.stats.bitrate = bitrate;
            }
            if let Ok(fps) = caps[3].parse::<f64>() {
                self.stats.fps = fps;
            }
            self.stats.update();
            return;
        }
        // Essayer le format aomenc
        if let Some(caps) = AOMENC_REGEX.captures(line) {
            if let Ok(pass) = caps[1].parse::<u32>() {
                self.stats.current_pass = pass;
            }
            if let Ok(total) = caps[2].parse::<u32>() {
                self.stats.total_passes = total;
            }
            if let Ok(frame) = caps[3].parse::<u64>() {
                self.stats.frame = frame;
            }
            if let Ok(fps) = caps[4].parse::<f64>() {
                self.stats.fps = fps;
            }
            self.stats.update();
        }
    }

    /// Réinitialiser les stats pour une nouvelle passe
    pub fn reset_for_pass(&mut self, pass: u32) {
        self.stats.frame = 0;
        self.stats.fps = 0.0;
        self.stats.bitrate = 0.0;
        self.stats.progress_percent = 0.0;
        self.stats.eta = None;
        self.stats.current_pass = pass;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_parse_ffmpeg_line() {
        let mut parser = StatsParser::new(Some(1000), None);

        // Format -progress (key=value sur des lignes séparées)
        parser.parse_line("frame=123");
        parser.parse_line("fps=25.3");
        parser.parse_line("bitrate=1234.5kbits/s");
        parser.parse_line("progress=continue");

        let stats = parser.get_stats();
        assert_eq!(stats.frame, 123);
        assert_eq!(stats.fps, 25.3);
        assert_eq!(stats.bitrate, 1234.5);
        // Note: out_time parsing nécessiterait un regex différent pour le format microseconde
    }
}
