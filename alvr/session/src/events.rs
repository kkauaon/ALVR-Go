// fixme: this module is misplaced. Find a way to resolve the mutual dependency with alvr_session

use crate::SessionDesc;
use alvr_common::log;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum EventSeverity {
    Error,
    Warning,
    Info,
    Debug,
}

// Sent by the C++ side every 100 ms (ClientConnection::ProcessTimeSync()). Fields are lenient
// on purpose: a single mismatch with the C++ JSON makes the whole event fail to parse and the
// statistics page would stop updating.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)] // todo: remove casing conversion
pub struct Statistics {
    pub total_packets: u64,
    pub packet_rate: u64,
    pub packets_lost_total: u64,
    pub packets_lost_per_second: u64,
    pub total_sent: u64,
    pub sent_rate: f32,
    pub bitrate: u64,
    pub ping: f32,
    pub total_latency: f32,
    pub encode_latency: f32,
    #[serde(alias = "sendLatency")]
    pub transport_latency: f32,
    pub decode_latency: f32,
    pub fec_percentage: u32,
    pub fec_failure_total: u64,
    pub fec_failure_in_second: u64,
    #[serde(rename = "clientFPS")]
    pub client_fps: f32,
    #[serde(rename = "serverFPS")]
    pub server_fps: f32,
    #[serde(rename = "batteryHMD")]
    pub battery_hmd: i32,
    pub battery_left: i32,
    pub battery_right: i32,
}

// This struct is temporary, until we switch to the new event system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Raw {
    pub timestamp: String,
    pub severity: EventSeverity,
    pub content: String,
}

// Event is serialized as #{ "id": "..." [, "data": ...] }#
// Pound signs are used to identify start and finish of json
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "id", content = "data")]
pub enum ServerEvent {
    Session(Box<SessionDesc>),
    SessionUpdated, // deprecated
    SessionSettingsExtrapolationFailed,
    ClientFoundOk,
    ClientFoundInvalid,
    ClientFoundWrongVersion(String),
    ClientConnected,
    ClientDisconnected,
    Statistics(Statistics),
    // Sent for every frame: [time, totalLatency, receiveLatency, renderTime, idleTime, waitTime,
    // encodeLatency, sendLatency, decodeLatency, clientIdleTime, clientFPS, serverFPS]
    GraphStatistics(Vec<f64>),
    ServerQuitting,
    Raw(Raw),
    EchoQuery(String),
}

pub fn log_event(id: ServerEvent) {
    log::info!("#{}#", serde_json::to_string(&id).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    // same format as the one written by ClientConnection::ProcessTimeSync()
    #[test]
    fn parses_statistics_sent_by_the_driver() {
        let line = r#"{ "id": "Statistics", "data": {"totalPackets": 1200, "packetRate": 900, "packetsLostTotal": 3, "packetsLostPerSecond": 0, "totalSent": 52, "sentRate": 31.250, "bitrate": 30, "ping": 2.500, "totalLatency": 45.125, "encodeLatency": 6.000, "sendLatency": 4.250, "decodeLatency": 9.000, "fecPercentage": 5, "fecFailureTotal": 1, "fecFailureInSecond": 0, "clientFPS": 71.500, "serverFPS": 72.000, "batteryHMD": 80, "batteryLeft": 0, "batteryRight": 55} }"#;
        let ServerEvent::Statistics(stats) = serde_json::from_str(line).unwrap() else {
            panic!("not a statistics event");
        };
        assert_eq!(stats.transport_latency, 4.25);
        assert_eq!(stats.client_fps, 71.5);
        assert_eq!(stats.battery_right, 55);

        let line = r#"{ "id": "GraphStatistics", "data": [123456,45.125,3.000,8.500,1.000,2.000,6.000,4.250,9.000,1.000,71.500,72.000] }"#;
        let ServerEvent::GraphStatistics(values) = serde_json::from_str(line).unwrap() else {
            panic!("not a graph statistics event");
        };
        assert_eq!(values[3], 8.5);
    }
}
