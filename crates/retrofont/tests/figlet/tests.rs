    use super::*;

    #[test]
    fn test_header() {
        let input = "flf2a$ 6 5 20 15 0 0 143 229";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);

        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), Some(229));
    }

    #[test]
    fn test_header_no_codetag() {
        let input = "flf2a$ 6 5 20 15 0 0 143";
        let mut reader: BufReader<&[u8]> = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);
        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), None);
    }

    #[test]
    fn test_header_no_full_layout() {
        let input = "flf2a$ 6 5 20 15 0 0";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.hard_blank_char(), '$');
        assert_eq!(header.height(), 6);
        assert_eq!(header.baseline(), 5);
        assert_eq!(header.max_length(), 20);
        assert_eq!(header.comment(), "");
        assert_eq!(header.print_direction(), PrintDirection::LeftToRight);

        assert_eq!(header.horiz_layout(), LayoutMode::Smushing);
        assert_eq!(
            header.horizontal_smushing(),
            HorizontalSmushing::EQUAL_CHARACTER | HorizontalSmushing::UNDERSCORE | HorizontalSmushing::HIERARCHY | HorizontalSmushing::OPPOSITE_PAIR
        );

        assert_eq!(header.vert_layout(), LayoutMode::Full);
        assert_eq!(header.vertical_smushing(), VerticalSmushing::NONE);

        assert_eq!(header.codetag_count(), None);
    }

    #[test]
    fn test_comments() {
        let input = "flf2a$ 6 5 20 15 3 0 143 229\nfoo\nbar\nbaz";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        assert_eq!(header.comment(), "foo\nbar\nbaz");
    }

    #[test]
    fn test_header_generation() {
        let input = "flf2a$ 6 5 20 15 0 0 143 229";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let generated = header.generate_string();
        assert_eq!(generated, input);
    }

    #[test]
    fn test_header_generation_comments() {
        let input = "flf2a$ 6 5 20 15 3 0 143 229\nfoo\nbar\nbaz";
        let mut reader = BufReader::new(input.as_bytes());
        let header = Header::read(&mut reader).unwrap();
        let generated = header.generate_string();
        assert_eq!(generated, input);
    }
