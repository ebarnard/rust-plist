use plist::plist;
fn dict() {
    ::plist::Value::Dictionary({
        let mut object = ::plist::Dictionary::new();
        let _ = object.insert(("a").into(), ::plist::Value::Boolean(true));
        let _ = object.insert(("b").into(), ::plist::to_value(&"astring").unwrap());
        let _ = object.insert(("c").into(), ::plist::to_value(&1).unwrap());
        let _ = object.insert(("d").into(), ::plist::to_value(&1.0).unwrap());
        let _ = object
            .insert(
                ("e").into(),
                ::plist::Value::Array(
                    <[_]>::into_vec(
                        #[rustc_box]
                        ::alloc::boxed::Box::new([
                            ::plist::to_value(&1).unwrap(),
                            ::plist::to_value(&2).unwrap(),
                            ::plist::to_value(&3).unwrap(),
                        ]),
                    ),
                ),
            );
        let _ = object
            .insert(
                ("f").into(),
                ::plist::Value::Dictionary({
                    let mut object = ::plist::Dictionary::new();
                    let _ = object.insert(("a").into(), ::plist::to_value(&1).unwrap());
                    let _ = object.insert(("b").into(), ::plist::to_value(&2).unwrap());
                    object
                }),
            );
        object
    });
}
