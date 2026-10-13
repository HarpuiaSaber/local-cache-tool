use futures::executor::block_on;
use rdkafka::ClientConfig;
use rdkafka::client::ClientContext;
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct ErrorContext {
    last_error: Arc<Mutex<Option<String>>>,
}

impl ClientContext for ErrorContext {
    fn error(&self, error: KafkaError, reason: &str) {
        let mut last_error = self.last_error.lock().unwrap();
        if last_error.is_none() || reason.to_ascii_lowercase().contains("authentication") {
            *last_error = Some(format!("{error}: {reason}"));
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    pub broker: String,
    pub topic: String,
    pub username: String,
    pub password: String,
}

impl Connection {
    pub fn validate(&self) -> Result<(), String> {
        if self.broker.trim().is_empty()
            || self.topic.trim().is_empty()
            || self.username.trim().is_empty()
            || self.password.is_empty()
        {
            return Err("Cần nhập broker, topic, username và password".into());
        }
        Ok(())
    }
}

pub fn send(config: Connection, payload: String, key: Option<&str>) -> Result<String, String> {
    config.validate()?;
    let last_error = Arc::new(Mutex::new(None));
    let producer: FutureProducer<ErrorContext> = ClientConfig::new()
        .set("bootstrap.servers", &config.broker)
        .set("security.protocol", "SASL_PLAINTEXT")
        .set("sasl.mechanism", "SCRAM-SHA-512")
        .set("sasl.username", &config.username)
        .set("sasl.password", &config.password)
        .set("message.timeout.ms", "10000")
        .create_with_context(ErrorContext {
            last_error: last_error.clone(),
        })
        .map_err(|error| format!("Không tạo được Kafka producer: {error}"))?;

    let record = FutureRecord::<str, str>::to(&config.topic).payload(&payload);
    let delivery = match key {
        Some(key) => producer.send(record.key(key), Duration::from_secs(10)),
        None => producer.send(record, Duration::from_secs(10)),
    };
    match block_on(delivery) {
        Ok(_) => Ok("Đã gửi message thành công".into()),
        Err((error, _)) => {
            let detail = last_error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| error.to_string());
            Err(format!("Gửi Kafka thất bại: {detail}"))
        }
    }
}

pub fn manual_key(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}

pub fn message_key(payload: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(payload).map_err(|error| format!("Message không hợp lệ: {error}"))?;
    value["target"]
        .as_str()
        .filter(|target| !target.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Message không có target hợp lệ".into())
}

pub fn test_connection(config: Connection) -> Result<String, String> {
    config.validate()?;
    let last_error = Arc::new(Mutex::new(None));
    let producer: FutureProducer<ErrorContext> = ClientConfig::new()
        .set("bootstrap.servers", &config.broker)
        .set("security.protocol", "SASL_PLAINTEXT")
        .set("sasl.mechanism", "SCRAM-SHA-512")
        .set("sasl.username", &config.username)
        .set("sasl.password", &config.password)
        .set("message.timeout.ms", "10000")
        .create_with_context(ErrorContext {
            last_error: last_error.clone(),
        })
        .map_err(|error| format!("Không tạo được Kafka producer: {error}"))?;

    producer
        .client()
        .fetch_metadata(None, Duration::from_secs(10))
        .map_err(|error| {
            last_error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| format!("Kết nối Kafka thất bại: {error}"))
        })?;
    Ok("Kết nối Kafka thành công".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_message_target_as_kafka_key() {
        assert_eq!(
            message_key(r#"{"target":"UserProfile"}"#).unwrap(),
            "UserProfile"
        );
    }

    #[test]
    fn omits_blank_manual_key() {
        assert_eq!(manual_key("   "), None);
        assert_eq!(manual_key("cache-42"), Some("cache-42".into()));
    }

    #[test]
    fn rejects_missing_connection_fields_before_network() {
        let connection = Connection {
            broker: "localhost:9093".into(),
            topic: "my-awesome-topic".into(),
            username: "alice".into(),
            password: String::new(),
        };
        assert!(connection.validate().is_err());
    }

    #[test]
    #[ignore = "requires a local Kafka broker and KAFKA_TEST_USERNAME/KAFKA_TEST_PASSWORD"]
    fn sends_to_local_scram_broker() {
        let config = Connection {
            broker: "localhost:9093".into(),
            topic: "my-awesome-topic".into(),
            username: std::env::var("KAFKA_TEST_USERNAME").unwrap(),
            password: std::env::var("KAFKA_TEST_PASSWORD").unwrap(),
        };
        let payload = r#"{"target":"ChannelConfig","action":"RELOAD","data":null,"changedAt":"2026-04-10T15:30:00+07:00"}"#;
        send(config, payload.into(), Some("ChannelConfig")).unwrap();
    }
}
