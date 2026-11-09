use super::stub::Stub;
use crate::commands::{clipboard_clear, clipboard_get, clipboard_set};
use crate::refs::refs_test_support::TempDir;
use crate::{
    ClipboardContent, ClipboardFormat, CommandContext, ImageBuffer, ImageFormat, InteractionPolicy,
};
use serde_json::json;

golden_test!(
    clipboard_get_text_file_urls_image_and_missing,
    include_str!(
        "../../../../../../tests/fixtures/contract/inventory-clipboard_get_text_file_urls_image_and_missing.json"
    ),
    &["path"],
    {
        let dir = TempDir::new();
        let read = |format, out, content| {
            let args = clipboard_get::ClipboardGetArgs { format, out };
            clipboard_get::execute(args, &Stub::holding(content), &CommandContext::default())
                .unwrap()
        };
        let urls = ClipboardContent::FileUrls(vec!["/tmp/a.txt".into()]);
        let image = ClipboardContent::Image(ImageBuffer {
            data: vec![1, 2, 3],
            format: ImageFormat::Png,
            width: 32,
            height: 16,
            scale_factor: 1.0,
        });
        json!([
            read(None, None, Some(ClipboardContent::Text("one\ntwo".into()))),
            read(Some(ClipboardFormat::FileUrls), None, Some(urls)),
            read(
                Some(ClipboardFormat::Image),
                Some(dir.path().join("clip.png")),
                Some(image)
            ),
            read(Some(ClipboardFormat::Auto), None, None),
        ])
    }
);

golden_test!(
    clipboard_set_text,
    include_str!("../../../../../../tests/fixtures/contract/inventory-clipboard_set_text.json"),
    &[],
    {
        let args = clipboard_set::ClipboardSetArgs {
            text: Some("hello".into()),
            image: None,
            file_urls: Vec::new(),
        };
        clipboard_set::execute(args, &Stub::default(), InteractionPolicy::headed()).unwrap()
    }
);

golden_test!(
    clipboard_clear,
    include_str!("../../../../../../tests/fixtures/contract/inventory-clipboard_clear.json"),
    &[],
    { clipboard_clear::execute(&Stub::default(), InteractionPolicy::headed()).unwrap() }
);
