# Kafka Message Tool

Windows 64-bit desktop app for sending cache-update JSON messages to Kafka.

## Build

Install Rust with [rustup](https://rustup.rs/), Visual Studio Build Tools with the **Desktop development with C++** workload, [CMake](https://cmake.org/download/), and [Strawberry Perl](https://strawberryperl.com/) for the vendored OpenSSL build. Add CMake and Perl to `PATH` during installation, then open a new PowerShell window and check `cmake --version` and `perl -v`. The current OpenSSL build uses `no-asm`, so NASM is not required. An internet connection is needed for the first `cargo` build so Cargo can download dependencies.

From this directory in PowerShell:

```powershell
cargo test
cargo build --release
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\package.ps1
```

The packaging script creates a ZIP under `dist/`. Copy that ZIP to another Windows 64-bit machine and extract it. The receiving machine does not need Rust. If Windows reports a missing Visual C++ runtime DLL, install the Microsoft Visual C++ Redistributable for x64 on that machine. Run the app from the extracted directory.

## Use

The app starts with `localhost:9093` and `my-awesome-topic`. Enter a username and password from your local Kafka setup; credentials remain in memory and are not written to disk. Select a target and action. Leave **Gửi data** unchecked for `"data": null`; otherwise check the fields to include, enter or select their values, review the JSON, and click **Gửi Kafka**. The app shows the Kafka delivery result.

`SASL_PLAINTEXT` authenticates but does not encrypt traffic. Use only with a broker configured for that protocol.

## Add a target or form

Add a `.json` file to `templates/` next to the executable, then restart the app. Each file defines one target and its available fields:

```json
{
  "target": "ChannelConfig",
  "fields": [
    { "name": "code", "kind": "text" },
    { "name": "type", "kind": "select", "options": ["FAST", "SLOW"] }
  ]
}
```

`kind` is `text` or `select`. `select` requires a nonempty `options` array. `value_type` is optional and defaults to `string`; set it to `boolean`, `integer`, or `number` when the JSON value must be a primitive rather than a string. A boolean only accepts `true` or `false`.

Use the optional `validation` object to constrain a value. Strings support `min_length` and `max_length`; integers support `min`, `max`, and `max_digits`; numbers support `min`, `max`, `max_integer_digits`, and `max_fraction_digits`. For example, this accepts values from `0` through `999.99`, with at most three digits before and two digits after the decimal point:

```json
{
  "name": "amount",
  "kind": "text",
  "value_type": "number",
  "validation": {
    "min": 0,
    "max": 999.99,
    "max_integer_digits": 3,
    "max_fraction_digits": 2
  }
}
```

A target with no data fields can use `"fields": []`. Target names and field names must be unique. The app displays a validation error if a file is malformed.

Kafka messages have no key. `changedAt` is set to the sending machine's local date and time with its UTC offset immediately before each send.

## Local Kafka authentication check

The broker must have SCRAM-SHA-512 credentials provisioned for the client user. In the local `apache/kafka` Compose setup, `user_admin` and `user_alice` in `kafka_server_jaas.conf` do not create SCRAM credentials; those `user_*` options belong to SASL/PLAIN. If the app reports `SASL authentication error: invalid credentials`, create the SCRAM users in Kafka metadata and check the entered password.

An optional integration test sends one message to the local topic. It is ignored by normal `cargo test` runs:

```powershell
$env:KAFKA_TEST_USERNAME = 'alice'
$env:KAFKA_TEST_PASSWORD = '<your-local-password>'
cargo test sends_to_local_scram_broker -- --ignored --nocapture
```
