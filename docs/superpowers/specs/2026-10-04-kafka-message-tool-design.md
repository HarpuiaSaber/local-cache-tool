# Kafka Message Tool Design

## Purpose

Build a Windows 64-bit desktop app in Rust and Slint that sends JSON cache-update messages to Kafka. The app starts with a local broker at `localhost:9093`, topic `my-awesome-topic`, SASL_PLAINTEXT, and SCRAM-SHA-512. The user enters the SASL username and password at runtime; the app does not save the password.

## Message format

Each message has exactly these top-level properties:

```json
{
  "target": "ChannelConfig",
  "action": "RELOAD",
  "data": { "code": "abc", "type": "FAST" },
  "changedAt": "2026-10-04T15:30:00+07:00"
}
```

`target` is chosen from loaded form definitions. `action` is one of INSERT, UPDATE, DELETE, and RELOAD; RELOAD is the initial selection. `data` is null when disabled or when no fields are selected. Otherwise it contains only selected fields. All field values in the first version are strings. `changedAt` is generated at send time as an ISO 8601 date-time with the machine's local UTC offset. Kafka records are sent without a key.

## Form definitions

The app loads JSON files from a `templates` directory next to its executable. Each file defines one target and zero or more fields. A field has a unique name and is either text input or a select input with a nonempty list of allowed string values. The initial `ChannelConfig` definition has `code` as text input and `type` as a select input with FAST and SLOW. The UI lets the user include either, both, or neither field. Additional targets and forms are added by adding a definition file, without rebuilding the app.

Example:

```json
{
  "target": "ChannelConfig",
  "fields": [
    { "name": "code", "kind": "text" },
    { "name": "type", "kind": "select", "options": ["FAST", "SLOW"] }
  ]
}
```

Text fields must be nonempty when selected. The app validates definitions on load and displays a useful error for invalid files rather than silently dropping them.

## User flow

The main window shows bootstrap server, topic, username, and password; target and action selectors; a data toggle and controls for the selected target's fields; a preview of the exact JSON to be sent; and a Send button. Broker and topic start with the local values above and can be edited. The password input is masked. The app does not prefill either account's credentials. The preview updates when selections or input change. Sending validates required connection fields and selected data fields, then generates a fresh `changedAt` and sends the previewed message. The UI reports broker acknowledgement or a specific error and remains responsive during network work.

## Kafka behavior

Use the Rust `rdkafka` crate with `security.protocol=SASL_PLAINTEXT`, `sasl.mechanism=SCRAM-SHA-512`, and the entered `sasl.username`/`sasl.password`. Send UTF-8 JSON to the entered topic and wait for delivery confirmation. The Java `sasl.jaas.config` string is not passed through; its username and password map to these two librdkafka settings. No Kafka key is used. Do not persist credentials or log them.

## Packaging and verification

Build a Windows x86-64 release executable and distribute it with the `templates` directory and any runtime files required by the selected dependencies. Document prerequisites and the exact build/package commands. Automated tests cover template validation and generation of messages with null, one-field, and multiple-field data, plus action and timestamp formatting. A local smoke test sends to the supplied Docker broker when it is reachable. If the local toolchain or broker is unavailable, report that limit explicitly rather than claiming the app was verified end to end.
