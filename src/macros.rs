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

/// Create a [`Value::Array`](crate::Value::Array) from a list of values
///
/// ## Example
///
/// ```
/// # use plist::{plist_array, Value};
/// let array = plist_array![1, 2];
/// assert_eq!(array, Value::Array(vec![Value::from(1), Value::from(2)]));
///
/// let other_array = plist_array!["hi"; 2];
/// assert_eq!(other_array, Value::Array(vec![Value::from("hi"), Value::from("hi")]));
/// ```
#[macro_export]
macro_rules! plist_array {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$($crate::plist_array!(@single $rest)),*]));

    ($($value:expr,)+) => { $crate::plist_array!($($value),+) };
    ($($value:expr),*) => {
        {
            let item_count = $crate::plist_array!(@count $($value),*);
            let mut _array = ::std::vec::Vec::with_capacity(item_count);
            $(
                _array.push($crate::Value::from($value));
            )*
            $crate::Value::Array(_array)
        }
    };

    ($value:expr; $n:expr) => ($crate::Value::Array(::std::vec![$crate::Value::from($value); $n]));
}

#[cfg(test)]
mod tests {
    use crate::Value;

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

    #[test]
    fn test_plist_array() {
        let digits = plist_array![1, 2, 3];
        let Value::Array(digits) = &digits else {
            panic!("wrong plist::Value variant, expected Value::Array, got {digits:?}");
        };
        assert_eq!(
            digits,
            &vec![Value::from(1), Value::from(2), Value::from(3)],
        );

        let repeated = plist_array![1; 5];
        let Value::Array(repeated) = &repeated else {
            panic!("wrong plist::Value variant, expected Value::Array, got {repeated:?}");
        };
        assert_eq!(repeated, &vec![Value::from(1); 5]);

        let empty = plist_array![];
        let Value::Array(empty) = &empty else {
            panic!("wrong plist::Value variant, expected Value::Array, got {empty:?}");
        };
        assert!(empty.is_empty());

        let _nested_compiles = plist_array![plist_array![1, 2, 3]];
    }
}
