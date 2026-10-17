use std::io::{ErrorKind, Write};
use std::process::{Command, Output, Stdio};
use std::thread;

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bfilter"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    // Feed and drain concurrently so tests can exceed the pipe buffer size.
    let feeder = thread::spawn(move || {
        if let Err(error) = stdin.write_all(&input) {
            assert_eq!(error.kind(), ErrorKind::BrokenPipe);
        }
    });
    let output = child.wait_with_output().unwrap();
    feeder.join().unwrap();
    output
}

fn assert_success(args: &[&str], input: &[u8], expected: &[u8]) {
    let output = run(args, input);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{args:?}: unexpected stderr");
    assert_eq!(output.stdout, expected, "{args:?}");
}

fn assert_failure(args: &[&str], input: &[u8], message: &str) {
    let output = run(args, input);
    assert!(!output.status.success(), "{args:?}: unexpectedly succeeded");
    assert!(output.stdout.is_empty(), "{args:?}: unexpected stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.is_empty(), "{args:?}: missing error message");
    assert!(stderr.contains(message), "{args:?}: {stderr}");
}

#[test]
fn set_preserves_surrounding_bytes_for_every_size_and_byte_order() {
    let cases: &[(&str, &str, &[u8])] = &[
        ("-b", "0x81", &[0x81]),
        ("-w", "0x1234", &[0x34, 0x12]),
        ("-d", "0x89abcdef", &[0xef, 0xcd, 0xab, 0x89]),
        (
            "-q",
            "0xfedcba9876543210",
            &[0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
        ),
    ];
    let input = [
        0x00, 0xff, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x0a,
    ];
    for &(size, value, little_endian) in cases {
        for order in [None, Some("--little-endian"), Some("--big-endian")] {
            let mut args = vec!["-s", size];
            args.extend(order);
            args.extend(["0x2", value]);
            let mut field = little_endian.to_vec();
            if order == Some("--big-endian") {
                field.reverse();
            }
            let mut expected = input;
            expected[2..2 + field.len()].copy_from_slice(&field);
            assert_success(&args, &input, &expected);
        }
    }
}

#[test]
fn get_extracts_raw_bytes_for_every_size() {
    let input = [0x99, 0x00, 0xff, 0x81, 0x42, 0x0a, 0x0d, 0xfe, 0x01, 0x77];
    for (size, width) in [("-b", 1), ("-w", 2), ("-d", 4), ("-q", 8)] {
        assert_success(&["-g", size, "1"], &input, &input[1..1 + width]);
        assert_success(&["-g", size, "0"], &input[..width], &input[..width]);
    }
    assert_success(&["-gq", "0"], &input, &input[..8]);
}

#[test]
fn offset_units_are_independent_of_field_size_and_byte_order() {
    let input: Vec<u8> = (0..32).collect();
    for (unit_flag, unit) in [("-B", 1), ("-W", 2), ("-D", 4), ("-Q", 8)] {
        let start = 2 * unit;
        for (size, width) in [("-b", 1), ("-w", 2), ("-d", 4), ("-q", 8)] {
            assert_success(
                &["-g", size, unit_flag, "0b10"],
                &input,
                &input[start..start + width],
            );
        }
        let mut expected = input.clone();
        expected[start..start + 2].copy_from_slice(&[0x12, 0x34]);
        assert_success(
            &["-s", "-w", unit_flag, "--big-endian", "0x2", "0x1234"],
            &input,
            &expected,
        );
    }
    assert_success(&["-gqB", "0"], &input, &input[..8]);
    assert_success(&["-gqQ", "2"], &input, &input[16..24]);
}

#[test]
fn masks_use_the_selected_size_and_byte_order() {
    let cases: &[(&str, &str, &[u8])] = &[
        ("-b", "0x0f", &[0x0f]),
        ("-w", "0x0ff0", &[0xf0, 0x0f]),
        ("-d", "0xff00f00f", &[0x0f, 0xf0, 0x00, 0xff]),
        (
            "-q",
            "0x800ff033ccaa55fe",
            &[0xfe, 0x55, 0xaa, 0xcc, 0x33, 0xf0, 0x0f, 0x80],
        ),
    ];
    let input = [0x99, 0xaa, 0x55, 0xf0, 0x0f, 0x12, 0x34, 0x56, 0x78, 0x88];
    for &(size, value, little_endian) in cases {
        for order in ["--little-endian", "--big-endian"] {
            let mut mask = little_endian.to_vec();
            if order == "--big-endian" {
                mask.reverse();
            }
            for operation in ["-a", "-o"] {
                let mut expected = input;
                for (byte, mask) in expected[1..].iter_mut().zip(&mask) {
                    *byte = if operation == "-a" {
                        *byte & mask
                    } else {
                        *byte | mask
                    };
                }
                assert_success(&[operation, size, order, "1", value], &input, &expected);
            }
        }
    }
}

#[test]
fn decimal_hexadecimal_and_binary_are_accepted_for_both_arguments() {
    for offset in ["2", "0x2", "0b10"] {
        for value in ["165", "0xa5", "0b10100101"] {
            assert_success(&["-s", "-b", offset, value], &[1, 2, 3, 4], &[1, 2, 165, 4]);
        }
    }
}

#[test]
fn mutations_extend_empty_input_and_gaps_with_zeros() {
    for (operation, field) in [("-s", [0x34, 0x12]), ("-a", [0, 0]), ("-o", [0x34, 0x12])] {
        assert_success(&[operation, "-w", "0", "0x1234"], &[], &field);
        assert_success(
            &[operation, "-w", "2", "0x1234"],
            &[],
            &[0, 0, field[0], field[1]],
        );
        assert_success(
            &[operation, "-w", "1", "0x1234"],
            &[0x99],
            &[0x99, field[0], field[1]],
        );
        assert_success(
            &[operation, "-w", "3", "0x1234"],
            &[0x99],
            &[0x99, 0, 0, field[0], field[1]],
        );
    }
}

#[test]
fn partial_fields_are_zero_filled_before_endian_conversion() {
    let input = [0x99, 0xab];
    for (operation, order, expected) in [
        ("-s", "--little-endian", [0x99, 0x34, 0x12]),
        ("-s", "--big-endian", [0x99, 0x12, 0x34]),
        ("-a", "--little-endian", [0x99, 0x20, 0x00]),
        ("-a", "--big-endian", [0x99, 0x02, 0x00]),
        ("-o", "--little-endian", [0x99, 0xbf, 0x12]),
        ("-o", "--big-endian", [0x99, 0xbb, 0x34]),
    ] {
        assert_success(&[operation, "-w", order, "1", "0x1234"], &input, &expected);
    }
}

#[test]
fn maximum_values_fit_in_each_size() {
    for (size, width, maximum) in [
        ("-b", 1, "255"),
        ("-w", 2, "65535"),
        ("-d", 4, "4294967295"),
        ("-q", 8, "18446744073709551615"),
    ] {
        for order in ["--little-endian", "--big-endian"] {
            for operation in ["-s", "-a", "-o"] {
                assert_success(
                    &[operation, size, order, "0", maximum],
                    &[0xff; 8][..width],
                    &[0xff; 8][..width],
                );
                assert_success(
                    &[operation, size, order, "0", "0"],
                    &[0; 8][..width],
                    &[0; 8][..width],
                );
            }
        }
    }
}

#[test]
fn get_rejects_short_input_without_writing_partial_bytes() {
    for (size, width) in [("-b", 1), ("-w", 2), ("-d", 4), ("-q", 8)] {
        assert_failure(
            &["-g", size, "0"],
            &[0xff; 8][..width - 1],
            "Input is too short",
        );
        assert_failure(&["-g", size, "4"], &[1, 2], "Input is too short");
        assert_failure(&["-g", size, "2"], &[1, 2], "Input is too short");
    }
}

#[test]
fn invalid_flag_combinations_and_missing_arguments_are_rejected() {
    let cases: &[&[&str]] = &[
        &[],
        &["-b", "0", "1"],
        &["-s", "0", "1"],
        &["-g", "-b"],
        &["-s", "-b", "0"],
        &["-a", "-b", "0"],
        &["-o", "-b", "0"],
        &["-g", "-b", "0", "1"],
        &["-s", "-g", "-b", "0", "1"],
        &["-a", "-o", "-b", "0", "1"],
        &["-g", "-b", "-w", "0"],
        &["-g", "-d", "-q", "0"],
        &["-g", "-b", "--little-endian", "0"],
        &["-g", "-b", "--big-endian", "0"],
        &["--little-endian", "-gq", "0"],
        &["--big-endian", "-gq", "0"],
        &["-gq", "--little-endian", "0"],
        &["-gq", "--big-endian", "0"],
        &["-g", "-b", "--little-endian", "--big-endian", "0"],
        &["-s", "-b", "--little-endian", "--big-endian", "0", "1"],
        &["-g", "-b", "-B", "-W", "0"],
        &["-g", "-b", "-W", "-D", "0"],
        &["-g", "-b", "-D", "-Q", "0"],
        &["-g", "-b", "-Q", "-B", "0"],
        &["-s", "-b", "0", "1", "2"],
    ];
    for args in cases {
        assert_failure(args, &[1, 2, 3], "error:");
    }
}

#[test]
fn invalid_numbers_and_values_that_exceed_the_size_are_rejected() {
    for invalid in [
        "",
        "xyz",
        "0x",
        "0b",
        "0xgg",
        "0b102",
        "1.5",
        "18446744073709551616",
    ] {
        assert_failure(&["-g", "-b", invalid], &[], "Invalid offset");
        assert_failure(&["-s", "-q", "0", invalid], &[], "Invalid value");
    }
    assert_failure(&["-g", "-b", "--", "-1"], &[], "Invalid offset");
    assert_failure(&["-s", "-q", "--", "0", "-1"], &[], "Invalid value");
    for (size, value) in [("-b", "256"), ("-w", "65536"), ("-d", "4294967296")] {
        for operation in ["-s", "-a", "-o"] {
            assert_failure(&[operation, size, "0", value], &[], "does not fit");
        }
    }
}

#[test]
fn offset_plus_size_overflow_is_rejected_before_processing_input() {
    assert_failure(
        &["-g", "-b", "18446744073709551615"],
        &[],
        "Offset plus size",
    );
    for operation in ["-s", "-a", "-o"] {
        assert_failure(
            &[operation, "-q", "18446744073709551608", "0"],
            &[],
            "Offset plus size",
        );
    }
}

#[test]
fn offset_scaling_checks_overflow_before_processing_input() {
    for (unit_flag, unit) in [("-W", 2), ("-D", 4), ("-Q", 8)] {
        let largest = (u64::MAX / unit).to_string();
        let overflowing = (u64::MAX / unit + 1).to_string();
        for operation in ["-g", "-s", "-a", "-o"] {
            let mut args = vec![operation, "-b", unit_flag, overflowing.as_str()];
            if operation != "-g" {
                args.push("255");
            }
            assert_failure(&args, &[1, 2, 3], "Offset times unit");
        }
        // The largest valid product must reach the input-length check.
        assert_failure(
            &["-g", "-b", unit_flag, &largest],
            &[],
            "Input is too short",
        );
        // A valid product can still overflow when the field width is added.
        assert_failure(&["-g", "-q", unit_flag, &largest], &[], "Offset plus size");
    }
}

#[test]
fn large_input_is_preserved_across_stream_buffer_boundaries() {
    let input: Vec<u8> = (0..262_144).map(|index| (index % 251) as u8).collect();
    let mut expected = input.clone();
    expected[65_533..65_541].copy_from_slice(&[0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe]);
    assert_success(
        &["-s", "-q", "65533", "0xfedcba9876543210"],
        &input,
        &expected,
    );
    assert_success(&["-g", "-q", "65533"], &input, &input[65_533..65_541]);
}

#[test]
fn help_and_version_are_available() {
    let help = run(&["--help"], &[]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("OFFSET"));
    assert_success(
        &["--version"],
        &[],
        concat!("bfilter ", env!("CARGO_PKG_VERSION"), "\n").as_bytes(),
    );
}

#[cfg(target_os = "linux")]
#[test]
fn output_errors_including_the_final_flush_return_failure() {
    // A non-newline byte stays buffered until main explicitly flushes stdout.
    for args in [["-s", "-b", "0", "1"], ["-s", "-b", "65536", "1"]] {
        let full = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_bfilter"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(full))
            .stderr(Stdio::piped())
            .output()
            .unwrap();
        assert!(!output.status.success(), "{args:?}: unexpectedly succeeded");
        assert!(!output.stderr.is_empty(), "{args:?}: missing I/O error");
    }
}
