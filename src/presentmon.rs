use crate::stats::Frame;
use anyhow::{Context, Result, bail};
use csv::StringRecord;

pub struct Schema {
    pid: usize,
    chain: usize,
    time: usize,
    time_divisor: f64,
    interval: usize,
}

impl Schema {
    pub fn new(header: &StringRecord) -> Result<Self> {
        let find = |name: &str| {
            header
                .iter()
                .position(|s| s.trim_start_matches('\u{feff}').eq_ignore_ascii_case(name))
                .with_context(|| format!("PresentMon CSV missing {name}; expected v1 metrics"))
        };
        let (time, time_divisor) = match find("TimeInSeconds") {
            Ok(ix) => (ix, 1.0),
            Err(_) => (find("TimeInMs")?, 1000.0),
        };
        Ok(Self {
            pid: find("ProcessID")?,
            chain: find("SwapChainAddress")?,
            time,
            time_divisor,
            interval: find("MsBetweenPresents")?,
        })
    }

    pub fn parse(&self, row: &StringRecord) -> Result<Option<Frame>> {
        let field = |ix| row.get(ix).context("incomplete PresentMon CSV row");
        let interval = field(self.interval)?;
        if interval == "NA" || interval.is_empty() {
            return Ok(None);
        }
        let frame_ms: f64 = interval.parse().context("invalid MsBetweenPresents")?;
        let time_s = field(self.time)?
            .parse::<f64>()
            .context("invalid frame timestamp")?
            / self.time_divisor;
        if !frame_ms.is_finite() || frame_ms <= 0.0 || !time_s.is_finite() || time_s < 0.0 {
            return Ok(None);
        }
        let pid = field(self.pid)?.parse().context("invalid ProcessID")?;
        let swapchain = field(self.chain)?.to_owned();
        if swapchain.is_empty() {
            bail!("empty swapchain")
        }
        Ok(Some(Frame {
            pid,
            swapchain,
            time_s,
            frame_ms,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_reordered_quoted_csv_and_filters_na() {
        let text = "SwapChainAddress,MsBetweenPresents,ProcessID,TimeInMs,Application\n0xA,16.5,42,1200,\"game, test.exe\"\n0xA,NA,42,1220,game.exe\n";
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        let schema = Schema::new(reader.headers().unwrap()).unwrap();
        let mut records = reader.records();
        let frame = schema
            .parse(&records.next().unwrap().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(frame.pid, 42);
        assert_eq!(frame.time_s, 1.2);
        assert_eq!(frame.frame_ms, 16.5);
        assert!(
            schema
                .parse(&records.next().unwrap().unwrap())
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn rejects_wrong_metric_schema_and_invalid_numbers() {
        assert!(Schema::new(&StringRecord::from(vec!["ProcessID", "FrameTime"])).is_err());
        let schema = Schema::new(&StringRecord::from(vec![
            "ProcessID",
            "SwapChainAddress",
            "TimeInSeconds",
            "MsBetweenPresents",
        ]))
        .unwrap();
        for value in ["NaN", "inf", "0", "-1", "NA"] {
            assert!(
                schema
                    .parse(&StringRecord::from(vec!["42", "0xA", "1", value]))
                    .unwrap()
                    .is_none()
            );
        }
    }
    #[test]
    fn parses_real_v1_lowercase_interval_header() {
        let schema = Schema::new(&StringRecord::from(vec![
            "ProcessID",
            "SwapChainAddress",
            "TimeInSeconds",
            "msBetweenPresents",
        ]))
        .unwrap();
        let frame = schema
            .parse(&StringRecord::from(vec!["42", "0xA", "2.5", "20"]))
            .unwrap()
            .unwrap();
        assert_eq!(frame.time_s, 2.5);
        assert_eq!(frame.frame_ms, 20.0);
    }
    #[test]
    fn timestamp_unit_follows_selected_normalized_header() {
        for label in ["\u{feff}TimeInMs", "timeinms", "TimeInMs"] {
            let schema = Schema::new(&StringRecord::from(vec![
                label,
                "ProcessID",
                "SwapChainAddress",
                "msBetweenPresents",
            ]))
            .unwrap();
            assert_eq!(
                schema
                    .parse(&StringRecord::from(vec!["2500", "42", "0xA", "20"]))
                    .unwrap()
                    .unwrap()
                    .time_s,
                2.5
            );
        }
        let schema = Schema::new(&StringRecord::from(vec![
            "TimeInSeconds",
            "TimeInMs",
            "ProcessID",
            "SwapChainAddress",
            "MsBetweenPresents",
        ]))
        .unwrap();
        assert_eq!(
            schema
                .parse(&StringRecord::from(vec!["2.5", "2500", "42", "0xA", "20"]))
                .unwrap()
                .unwrap()
                .time_s,
            2.5
        );
    }
}
