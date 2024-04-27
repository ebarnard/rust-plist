/// Create a [`Dictionary`](crate::Dictionary) from a list of key-value pairs
///
/// ## Example
///
/// ```
/// # use plist::{plist_dict, Value};
/// let map = plist_dict! {
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], Value::from(1));
/// assert_eq!(map["b"], Value::from(2));
/// assert_eq!(map.get("c"), None);
/// ```
#[macro_export]
macro_rules! plist_dict {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$($crate::plist_dict!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { $crate::plist_dict!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let item_count = $crate::plist_dict!(@count $($key),*);
            let mut _dict = $crate::Dictionary::with_capacity(item_count);
            $(
                let _ = _dict.insert(::std::string::String::from($key), $crate::Value::from($value));
            )*
            _dict
        }
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_plist_dict() {
        let digits = plist_dict! {
            "one" => 1,
            "two" => 2,
        };
        assert_eq!(digits.len(), 2);
        assert_eq!(digits["one"], 1.into());
        assert_eq!(digits["two"], 2.into());

        let empty = plist_dict! {};
        assert!(empty.is_empty());

        let _nested_compiles = plist_dict! {
            "inner" => plist_dict! {
                "one" => 1,
                "two" => 2,
            },
        };
    }
}
