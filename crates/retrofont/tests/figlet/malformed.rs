use retrofont::{FontError, figlet::FigletFont};

#[test]
fn incomplete_required_character_set_is_rejected() {
    let bytes = b"flf2a$ 1 1 1 0 0\n@@\n";

    assert!(matches!(
        FigletFont::load(bytes),
        Err(FontError::FigletIncompleteChar)
    ));
}

#[test]
fn truncated_comments_are_rejected() {
    let bytes = b"flf2a$ 1 1 1 0 2\nonly one comment\n";

    assert!(matches!(
        FigletFont::load(bytes),
        Err(FontError::FigletIncompleteComments)
    ));
}

#[test]
fn unsupported_height_is_rejected() {
    let bytes = b"flf2a$ 256 1 1 0 0\n";

    assert!(matches!(
        FigletFont::load(bytes),
        Err(FontError::FigletHeightOutOfRange {
            height: 256,
            max: 255
        })
    ));
}

#[test]
fn early_final_marker_is_rejected() {
    let bytes = b"flf2a$ 2 1 1 0 0\n@@\n";

    assert!(matches!(
        FigletFont::load(bytes),
        Err(FontError::FigletIncompleteChar)
    ));
}
