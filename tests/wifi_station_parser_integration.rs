use rust_stats::WifiStationParser;

#[test]
fn parses_station_dump_snapshot_from_watch_output() {
    let input = [
        "\u{1b}[H\u{1b}[2J\u{1b}[3JEvery 1.0s: iw dev wlan0 station dump",
        "",
        "Station 3a:98:b5:31:9d:34 (on wlan0)",
        "        inactive time:  0 ms",
        "        rx bytes:       530085722",
        "        rx packets:     573256",
        "        tx bytes:       21264393",
        "        tx packets:     51589",
        "        tx failed:      44",
        "        signal:         -36 dBm",
        "        tx bitrate:     65.0 MBit/s",
        "        rx bitrate:     72.2 MBit/s",
        "        authorized:     yes",
        "        authenticated:  yes",
        "        associated:     yes",
        "        WMM/WME:        yes",
        "        TDLS peer:      no",
        "        DTIM period:    3",
        "        beacon interval:108",
        "        short preamble: yes",
        "        short slot time:yes",
        "        connected time: 2191 seconds",
        "        current time:   1777675105798 ms",
    ];

    let mut parser = WifiStationParser::new();
    let mut parsed = None;

    for line in input {
        if let Some(sample) = parser.parse_line(line) {
            parsed = Some(sample);
        }
    }

    let sample = parsed.expect("expected one parsed wifi station sample");

    assert_eq!(sample.station_mac, "3a:98:b5:31:9d:34");
    assert_eq!(sample.interface_name, "wlan0");
    assert_eq!(sample.inactive_time_ms, 0);
    assert_eq!(sample.rx_bytes, 530085722);
    assert_eq!(sample.tx_failed, 44);
    assert_eq!(sample.signal_dbm, -36);
    assert_eq!(sample.tx_bitrate_mbit_s, 65.0);
    assert_eq!(sample.rx_bitrate_mbit_s, 72.2);
    assert!(sample.authorized);
    assert!(sample.authenticated);
    assert!(sample.associated);
    assert!(!sample.tdls_peer);
    assert_eq!(sample.connected_time_seconds, 2191);
    assert_eq!(sample.current_time_ms, 1777675105798);
}

#[test]
fn parses_station_dump_snapshot_from_plain_iw_output() {
    let input = [
        "Station 3a:98:b5:31:9d:34 (on wlan0)",
        "\tinactive time:\t0 ms",
        "\trx bytes:\t530085722",
        "\trx packets:\t573256",
        "\ttx bytes:\t21264393",
        "\ttx packets:\t51589",
        "\ttx failed:\t44",
        "\tsignal:\t-36 dBm",
        "\ttx bitrate:\t65.0 MBit/s",
        "\trx bitrate:\t72.2 MBit/s",
        "\tauthorized:\tyes",
        "\tauthenticated:\tyes",
        "\tassociated:\tyes",
        "\tWMM/WME:\tyes",
        "\tTDLS peer:\tno",
        "\tDTIM period:\t3",
        "\tbeacon interval:\t108",
        "\tshort preamble:\tyes",
        "\tshort slot time:\tyes",
        "\tconnected time:\t2191 seconds",
        "\tcurrent time:\t1777675105798 ms",
    ];

    let mut parser = WifiStationParser::new();
    let mut parsed = None;

    for line in input {
        if let Some(sample) = parser.parse_line(line) {
            parsed = Some(sample);
        }
    }

    let sample = parsed.expect("expected one parsed wifi station sample");

    assert_eq!(sample.station_mac, "3a:98:b5:31:9d:34");
    assert_eq!(sample.interface_name, "wlan0");
    assert_eq!(sample.signal_dbm, -36);
    assert_eq!(sample.tx_bitrate_mbit_s, 65.0);
    assert_eq!(sample.rx_bitrate_mbit_s, 72.2);
    assert_eq!(sample.current_time_ms, 1777675105798);
}
