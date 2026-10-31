#[allow(dead_code)]
mod traverse {
    include!("../track-d/src/main.rs");

    #[test]
    fn records_across_every_buffer_boundary() {
        let bytes = b" \t0 4294967295 00042\r\n1\t2 3\n4 5 6";
        for capacity in 1..=bytes.len() {
            let mut reader = BufReader::with_capacity(capacity, &bytes[..]);
            let mut buffer = Vec::new();
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([0, u32::MAX, 42])
            );
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([1, 2, 3])
            );
            assert_eq!(
                read_record(&mut reader, &mut buffer).unwrap(),
                Some([4, 5, 6])
            );
            assert_eq!(read_record::<3>(&mut reader, &mut buffer).unwrap(), None);
        }
    }

    #[test]
    fn rejects_malformed_or_overflowing_fields() {
        for bytes in [
            &b"\n"[..],
            &b" \t\r\n"[..],
            &b"1"[..],
            &b"1 2 3"[..],
            &b"-1 2"[..],
            &b"1 +2"[..],
            &b"1 2x"[..],
            &b"1 4294967296"[..],
            &b"999999999999999999999999999999 2"[..],
            &b"1 \xff"[..],
            &b"1.0 2"[..],
            &b"1\x002"[..],
        ] {
            let error = read_record::<2>(&mut &bytes[..], &mut Vec::new()).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{bytes:?}");
        }
    }

    #[test]
    fn empty_input_is_eof() {
        assert_eq!(
            read_record::<2>(&mut &b""[..], &mut Vec::new()).unwrap(),
            None
        );
    }

    #[test]
    fn long_records_reuse_buffer() {
        let bytes = format!("{}1 2\n3 4\n", " ".repeat(32_768));
        let mut reader = BufReader::with_capacity(7, bytes.as_bytes());
        let mut buffer = Vec::new();
        assert_eq!(read_record(&mut reader, &mut buffer).unwrap(), Some([1, 2]));
        let capacity = buffer.capacity();
        assert_eq!(read_record(&mut reader, &mut buffer).unwrap(), Some([3, 4]));
        assert_eq!(buffer.capacity(), capacity);
    }

    #[test]
    fn propagates_read_failure_after_partial_record() {
        struct Fails;
        impl io::Read for Fails {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected failure"))
            }
        }
        let mut reader = BufReader::new(io::Read::chain(&b"1 "[..], Fails));
        let error = read_record::<2>(&mut reader, &mut Vec::new()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }
}
