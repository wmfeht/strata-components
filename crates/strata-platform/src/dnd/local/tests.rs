use super::*;

#[test]
fn each_drag_gets_its_own_token_naming_this_process() {
    let (a, b) = (new_token(), new_token());
    assert_ne!(a, b);
    let pid = std::process::id().to_string();
    for t in [&a, &b] {
        assert!(is_token(t), "{t}");
        let id = t
            .strip_prefix("application/x-local-drag;id=")
            .expect("prefix");
        let parts: Vec<&str> = id.split('.').collect();
        assert_eq!(parts.len(), 3, "{t}");
        assert_eq!(parts[0], pid);
    }
}

#[test]
fn tokens_are_hidden_from_what_an_app_sees() {
    let mimes = vec![
        "text/uri-list".to_owned(),
        new_token(),
        "application/x-local-dragged-thing".to_owned(),
    ];
    assert_eq!(token_in(&mimes), Some(mimes[1].as_str()));
    assert_eq!(
        visible(&mimes),
        vec!["text/uri-list", "application/x-local-dragged-thing"],
        "a type that only starts like the token stays"
    );
}
