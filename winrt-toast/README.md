# winrt-toast
A mostly usable binding to the Windows `ToastNotification` API.

## Example
```rust
use winrt_toast::{Toast, Text, Header, ToastManager};
use winrt_toast::content::text::TextPlacement;

let manager = ToastManager::new(crate::AUM_ID);

let mut toast = Toast::new();
toast
    .text1("Title")
    .text2(Text::new("Body"))
    .text3(
        Text::new("Via SMS")
            .with_placement(TextPlacement::Attribution)
    );

manager.show(&toast).expect("Failed to show toast");

// Or you may add callbacks
manager.show_with_callbacks(
    &toast, None, None,
    Some(Box::new(move |e| {
        // This will be called if Windows fails to show the toast.
        eprintln!("Failed to show toast: {:?}", e);
    }))
).expect("Failed to show toast");
```

## To-Do Features
* [ ] Actions
* [ ] Better callbacks
* [ ] Sound
* [ ] Adaptive contents and data binding