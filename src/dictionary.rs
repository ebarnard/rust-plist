//! A map of String to plist::Value.
//!
//! The map is currently backed by an [`IndexMap`]. This may be changed in a future minor release.
//!
//! [`IndexMap`]: https://docs.rs/indexmap/latest/indexmap/map/struct.IndexMap.html

use indexmap::{map, IndexMap};
use serde::{de, ser};
use std::{
    fmt::{self, Debug},
    iter::FromIterator,
    ops,
};

use crate::Value;

/// Represents a plist dictionary type.
pub struct Dictionary {
    map: IndexMap<String, Value>,
}

impl Dictionary {
    /// Makes a new empty `Dictionary`.
    #[inline]
    pub fn new() -> Self {
        Dictionary {
            map: IndexMap::new(),
        }
    }

    /// Clears the dictionary, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear()
    }

    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    /// Returns true if the dictionary contains a value for the specified key.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.map.get_mut(key)
    }

    /// Inserts a key-value pair into the dictionary.
    ///
    /// If the dictionary did not have this key present, `None` is returned.
    ///
    /// If the dictionary did have this key present, the value is updated, and the old value is
    /// returned.
    #[inline]
    pub fn insert(&mut self, k: String, v: Value) -> Option<Value> {
        self.map.insert(k, v)
    }

    /// Removes a key from the dictionary, returning the value at the key if the key was previously
    /// in the dictionary.
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.map.remove(key)
    }

    /// Scan through each key-value pair in the map and keep those where the
    /// closure `keep` returns `true`.
    #[inline]
    pub fn retain<F>(&mut self, keep: F)
    where
        F: FnMut(&String, &mut Value) -> bool,
    {
        self.map.retain(keep)
    }

    /// Sort the dictionary keys.
    ///
    /// This uses the default ordering defined on [`str`].
    ///
    /// This function is useful if you are serializing to XML, and wish to
    /// ensure a consistent key order.
    #[inline]
    pub fn sort_keys(&mut self) {
        self.map.sort_keys()
    }

    /// Gets the given key's corresponding entry in the dictionary for in-place manipulation.
    // Entry functionality is unstable until I can figure out how to use either Cow<str> or
    // T: AsRef<str> + Into<String>
    #[cfg(any(
        test,
        feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
    ))]
    pub fn entry<S>(&mut self, key: S) -> Entry
    where
        S: Into<String>,
    {
        match self.map.entry(key.into()) {
            map::Entry::Vacant(vacant) => Entry::Vacant(VacantEntry { vacant }),
            map::Entry::Occupied(occupied) => Entry::Occupied(OccupiedEntry { occupied }),
        }
    }

    /// Returns the number of elements in the dictionary.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the dictionary contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Gets an iterator over the entries of the dictionary.
    #[inline]
    pub fn iter(&self) -> Iter {
        Iter {
            iter: self.map.iter(),
        }
    }

    /// Gets a mutable iterator over the entries of the dictionary.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }

    /// Gets an iterator over the keys of the dictionary.
    #[inline]
    pub fn keys(&self) -> Keys {
        Keys {
            iter: self.map.keys(),
        }
    }

    /// Gets an iterator over the values of the dictionary.
    #[inline]
    pub fn values(&self) -> Values {
        Values {
            iter: self.map.values(),
        }
    }

    /// Gets an iterator over mutable values of the dictionary.
    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut {
        ValuesMut {
            iter: self.map.values_mut(),
        }
    }
}

impl Default for Dictionary {
    #[inline]
    fn default() -> Self {
        Dictionary {
            map: Default::default(),
        }
    }
}

impl Clone for Dictionary {
    #[inline]
    fn clone(&self) -> Self {
        Dictionary {
            map: self.map.clone(),
        }
    }
}

impl PartialEq for Dictionary {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map.eq(&other.map)
    }
}

/// Access an element of this dictionary. Panics if the given key is not present in the dictionary.
///
/// ```
/// # use plist::Value;
/// #
/// # let val = &Value::String("".to_owned());
/// # let _ =
/// match *val {
///     Value::Array(ref arr) => arr[0].as_string(),
///     Value::Dictionary(ref dict) => dict["type"].as_string(),
///     Value::String(ref s) => Some(s.as_str()),
///     _ => None,
/// }
/// # ;
/// ```
impl<'a> ops::Index<&'a str> for Dictionary {
    type Output = Value;

    fn index(&self, index: &str) -> &Value {
        self.map.index(index)
    }
}

/// Mutably access an element of this dictionary. Panics if the given key is not present in the
/// dictionary.
///
/// ```
/// # let mut dict = plist::Dictionary::new();
/// # dict.insert("key".to_owned(), plist::Value::Boolean(false));
/// #
/// dict["key"] = "value".into();
/// ```
impl<'a> ops::IndexMut<&'a str> for Dictionary {
    fn index_mut(&mut self, index: &str) -> &mut Value {
        self.map.get_mut(index).expect("no entry found for key")
    }
}

impl Debug for Dictionary {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.map.fmt(formatter)
    }
}

impl ser::Serialize for Dictionary {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map.serialize_key(k)?;
            map.serialize_value(v)?;
        }
        map.end()
    }
}

impl<'de> de::Deserialize<'de> for Dictionary {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Dictionary;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Dictionary::new())
            }

            #[inline]
            fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut values = Dictionary::new();

                while let Some((key, value)) = visitor.next_entry()? {
                    values.insert(key, value);
                }

                Ok(values)
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

impl FromIterator<(String, Value)> for Dictionary {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (String, Value)>,
    {
        Dictionary {
            map: FromIterator::from_iter(iter),
        }
    }
}

impl Extend<(String, Value)> for Dictionary {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (String, Value)>,
    {
        self.map.extend(iter);
    }
}

macro_rules! delegate_iterator {
    (($name:ident $($generics:tt)*) => $item:ty) => {
        impl $($generics)* Iterator for $name $($generics)* {
            type Item = $item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        /*impl $($generics)* DoubleEndedIterator for $name $($generics)* {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.iter.next_back()
            }
        }*/

        impl $($generics)* ExactSizeIterator for $name $($generics)* {
            #[inline]
            fn len(&self) -> usize {
                self.iter.len()
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

/// A view into a single entry in a dictionary, which may either be vacant or occupied.
/// This enum is constructed from the [`entry`] method on [`Dictionary`].
///
/// [`entry`]: struct.Dictionary.html#method.entry
/// [`Dictionary`]: struct.Dictionary.html
#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
pub enum Entry<'a> {
    /// A vacant Entry.
    Vacant(VacantEntry<'a>),
    /// An occupied Entry.
    Occupied(OccupiedEntry<'a>),
}

/// A vacant Entry. It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
pub struct VacantEntry<'a> {
    vacant: map::VacantEntry<'a, String, Value>,
}

/// An occupied Entry. It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
pub struct OccupiedEntry<'a> {
    occupied: map::OccupiedEntry<'a, String, Value>,
}

#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
impl<'a> Entry<'a> {
    /// Returns a reference to this entry's key.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut dict = plist::Dictionary::new();
    /// assert_eq!(dict.entry("serde").key(), &"serde");
    /// ```
    pub fn key(&self) -> &String {
        match *self {
            Entry::Vacant(ref e) => e.key(),
            Entry::Occupied(ref e) => e.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns a mutable
    /// reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut dict = plist::Dictionary::new();
    /// dict.entry("serde").or_insert(12.into());
    ///
    /// assert_eq!(dict["serde"], 12.into());
    /// ```
    pub fn or_insert(self, default: Value) -> &'a mut Value {
        match self {
            Entry::Vacant(entry) => entry.insert(default),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut dict = plist::Dictionary::new();
    /// dict.entry("serde").or_insert_with(|| "hoho".into());
    ///
    /// assert_eq!(dict["serde"], "hoho".into());
    /// ```
    pub fn or_insert_with<F>(self, default: F) -> &'a mut Value
    where
        F: FnOnce() -> Value,
    {
        match self {
            Entry::Vacant(entry) => entry.insert(default()),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }
}

#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
impl<'a> VacantEntry<'a> {
    /// Gets a reference to the key that would be used when inserting a value through the
    /// VacantEntry.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    ///
    /// match dict.entry("serde") {
    ///     Entry::Vacant(vacant) => assert_eq!(vacant.key(), &"serde"),
    ///     Entry::Occupied(_) => unimplemented!(),
    /// }
    /// ```
    #[inline]
    pub fn key(&self) -> &String {
        self.vacant.key()
    }

    /// Sets the value of the entry with the VacantEntry's key, and returns a mutable reference
    /// to it.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    ///
    /// match dict.entry("serde") {
    ///     Entry::Vacant(vacant) => vacant.insert("hoho".into()),
    ///     Entry::Occupied(_) => unimplemented!(),
    /// };
    /// ```
    #[inline]
    pub fn insert(self, value: Value) -> &'a mut Value {
        self.vacant.insert(value)
    }
}

#[cfg(any(
    test,
    feature = "enable_unstable_features_that_may_break_with_minor_version_bumps"
))]
impl<'a> OccupiedEntry<'a> {
    /// Gets a reference to the key in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), 12.into());
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(occupied) => assert_eq!(occupied.key(), &"serde"),
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    /// ```
    #[inline]
    pub fn key(&self) -> &String {
        self.occupied.key()
    }

    /// Gets a reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::Value;
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), 12.into());
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(occupied) => assert_eq!(occupied.get(), &Value::from(12)),
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    /// ```
    #[inline]
    pub fn get(&self) -> &Value {
        self.occupied.get()
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::Value;
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), Value::Array(vec![1.into(), 2.into(), 3.into()]));
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(mut occupied) => {
    ///         occupied.get_mut().as_array_mut().unwrap().push(4.into());
    ///     }
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    ///
    /// assert_eq!(dict["serde"].as_array().unwrap().len(), 4);
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> &mut Value {
        self.occupied.get_mut()
    }

    /// Converts the entry into a mutable reference to its value.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::Value;
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), Value::Array(vec![1.into(), 2.into(), 3.into()]));
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(mut occupied) => {
    ///         occupied.into_mut().as_array_mut().unwrap().push(4.into());
    ///     }
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    ///
    /// assert_eq!(dict["serde"].as_array().unwrap().len(), 4);
    /// ```
    #[inline]
    pub fn into_mut(self) -> &'a mut Value {
        self.occupied.into_mut()
    }

    /// Sets the value of the entry with the `OccupiedEntry`'s key, and returns
    /// the entry's old value.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::Value;
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), 12.into());
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(mut occupied) => {
    ///         assert_eq!(occupied.insert(13.into()), 12.into());
    ///         assert_eq!(occupied.get(), &Value::from(13));
    ///     }
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    /// ```
    #[inline]
    pub fn insert(&mut self, value: Value) -> Value {
        self.occupied.insert(value)
    }

    /// Takes the value of the entry out of the dictionary, and returns it.
    ///
    /// # Examples
    ///
    /// ```
    /// use plist::dictionary::Entry;
    ///
    /// let mut dict = plist::Dictionary::new();
    /// dict.insert("serde".to_owned(), 12.into());
    ///
    /// match dict.entry("serde") {
    ///     Entry::Occupied(occupied) => assert_eq!(occupied.remove(), 12.into()),
    ///     Entry::Vacant(_) => unimplemented!(),
    /// }
    /// ```
    #[inline]
    pub fn remove(self) -> Value {
        self.occupied.remove()
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a> IntoIterator for &'a Dictionary {
    type Item = (&'a String, &'a Value);
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.map.iter(),
        }
    }
}

/// An iterator over a plist::Dictionary's entries.
pub struct Iter<'a> {
    iter: IterImpl<'a>,
}

type IterImpl<'a> = map::Iter<'a, String, Value>;

delegate_iterator!((Iter<'a>) => (&'a String, &'a Value));

//////////////////////////////////////////////////////////////////////////////

impl<'a> IntoIterator for &'a mut Dictionary {
    type Item = (&'a String, &'a mut Value);
    type IntoIter = IterMut<'a>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }
}

/// A mutable iterator over a plist::Dictionary's entries.
pub struct IterMut<'a> {
    iter: map::IterMut<'a, String, Value>,
}

delegate_iterator!((IterMut<'a>) => (&'a String, &'a mut Value));

//////////////////////////////////////////////////////////////////////////////

impl IntoIterator for Dictionary {
    type Item = (String, Value);
    type IntoIter = IntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: self.map.into_iter(),
        }
    }
}

/// An owning iterator over a plist::Dictionary's entries.
pub struct IntoIter {
    iter: map::IntoIter<String, Value>,
}

delegate_iterator!((IntoIter) => (String, Value));

//////////////////////////////////////////////////////////////////////////////

/// An iterator over a plist::Dictionary's keys.
pub struct Keys<'a> {
    iter: map::Keys<'a, String, Value>,
}

delegate_iterator!((Keys<'a>) => &'a String);

//////////////////////////////////////////////////////////////////////////////

/// An iterator over a plist::Dictionary's values.
pub struct Values<'a> {
    iter: map::Values<'a, String, Value>,
}

delegate_iterator!((Values<'a>) => &'a Value);

//////////////////////////////////////////////////////////////////////////////

/// A mutable iterator over a plist::Dictionary's values.
pub struct ValuesMut<'a> {
    iter: map::ValuesMut<'a, String, Value>,
}

delegate_iterator!((ValuesMut<'a>) => &'a mut Value);

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Cursor, path::Path};

    use crate::{Date, Integer};

    use super::*;

    #[test]
    fn deserialize_dictionary_xml() {
        let reader = File::open(&Path::new("./tests/data/xml.plist")).unwrap();
        let dict: Dictionary = crate::from_reader(reader).unwrap();

        check_common_plist(&dict);

        // xml.plist has this member, but binary.plist does not.
        assert_eq!(
            dict.get("HexademicalNumber") // sic
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            0xDEADBEEF
        );
    }

    #[test]
    fn deserialize_dictionary_binary() {
        let reader = File::open(&Path::new("./tests/data/binary.plist")).unwrap();
        let dict: Dictionary = crate::from_reader(reader).unwrap();

        check_common_plist(&dict);
    }

    // Shared checks used by the tests deserialize_dictionary_xml() and
    // deserialize_dictionary_binary(), which load files with different formats
    // but the same data elements.
    fn check_common_plist(dict: &Dictionary) {
        // Array elements

        let lines = dict.get("Lines").unwrap().as_array().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0].as_string().unwrap(),
            "It is a tale told by an idiot,"
        );
        assert_eq!(
            lines[1].as_string().unwrap(),
            "Full of sound and fury, signifying nothing."
        );

        // Dictionary
        //
        // There is no embedded dictionary in this plist.  See
        // deserialize_dictionary_binary_nskeyedarchiver() below for an example
        // of that.

        // Boolean elements

        assert_eq!(dict.get("IsTrue").unwrap().as_boolean().unwrap(), true);

        assert_eq!(dict.get("IsNotFalse").unwrap().as_boolean().unwrap(), false);

        // Data

        let data = dict.get("Data").unwrap().as_data().unwrap();
        assert_eq!(data.len(), 15);
        assert_eq!(
            data,
            &[0, 0, 0, 0xbe, 0, 0, 0, 0x03, 0, 0, 0, 0x1e, 0, 0, 0]
        );

        // Date

        // TODO: Dates are being deserialized as a Value::String rather than as
        // Value::Date.  This should be fixed, but for now, just verify we get
        // the expected string value.

        // assert_eq!(
        //     dict.get("Birthdate")
        //         .unwrap()
        //         .as_date()
        //         .unwrap()
        //         .to_rfc3339(),
        //     "1981-05-16T11:32:06Z"
        // );
        assert_eq!(
            dict.get("Birthdate").unwrap().as_string().unwrap(),
            "1981-05-16T11:32:06Z"
        );

        // Real

        assert_eq!(dict.get("Height").unwrap().as_real().unwrap(), 1.6);

        // Integer

        assert_eq!(
            dict.get("BiggestNumber")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            18446744073709551615
        );

        assert_eq!(
            dict.get("Death").unwrap().as_unsigned_integer().unwrap(),
            1564
        );

        assert_eq!(
            dict.get("SmallestNumber")
                .unwrap()
                .as_signed_integer()
                .unwrap(),
            -9223372036854775808
        );

        // String

        assert_eq!(
            dict.get("Author").unwrap().as_string().unwrap(),
            "William Shakespeare"
        );

        assert_eq!(dict.get("Blank").unwrap().as_string().unwrap(), "");

        // Uid
        //
        // No checks for Uid value type in this test. See
        // deserialize_dictionary_binary_nskeyedarchiver() below for an example
        // of that.
    }

    #[test]
    fn deserialize_dictionary_binary_nskeyedarchiver() {
        let reader = File::open(&Path::new("./tests/data/binary_NSKeyedArchiver.plist")).unwrap();
        let dict: Dictionary = crate::from_reader(reader).unwrap();

        assert_eq!(
            dict.get("$archiver").unwrap().as_string().unwrap(),
            "NSKeyedArchiver"
        );

        // TODO: All UID values are being deserialized as Value::Integer rather
        // than Value::Uid.  This needs to be fixed, but for now we just verify
        // we are getting the expected integer values.

        let objects = dict.get("$objects").unwrap().as_array().unwrap();
        assert_eq!(objects.len(), 5);

        assert_eq!(objects[0].as_string().unwrap(), "$null");

        let objects_1 = objects[1].as_dictionary().unwrap();
        assert_eq!(
            objects_1
                .get("$class")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            4
        );
        assert_eq!(
            objects_1
                .get("NSRangeCount")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            42
        );
        assert_eq!(
            objects_1
                .get("NSRangeData")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            2
        );

        let objects_2 = objects[2].as_dictionary().unwrap();
        assert_eq!(
            objects_2
                .get("$class")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            3
        );
        let objects_2_nsdata = objects_2.get("NS.data").unwrap().as_data().unwrap();
        assert_eq!(objects_2_nsdata.len(), 103);
        assert_eq!(objects_2_nsdata[0], 0x03);
        assert_eq!(objects_2_nsdata[102], 0x01);

        let objects_3 = objects[3].as_dictionary().unwrap();
        let objects_3_classes = objects_3.get("$classes").unwrap().as_array().unwrap();
        assert_eq!(objects_3_classes.len(), 3);
        assert_eq!(objects_3_classes[0].as_string().unwrap(), "NSMutableData");
        assert_eq!(objects_3_classes[1].as_string().unwrap(), "NSData");
        assert_eq!(objects_3_classes[2].as_string().unwrap(), "NSObject");
        assert_eq!(
            objects_3.get("$classname").unwrap().as_string().unwrap(),
            "NSMutableData"
        );

        let objects_4 = objects[4].as_dictionary().unwrap();
        let objects_4_classes = objects_4.get("$classes").unwrap().as_array().unwrap();
        assert_eq!(objects_4_classes.len(), 3);
        assert_eq!(
            objects_4_classes[0].as_string().unwrap(),
            "NSMutableIndexSet"
        );
        assert_eq!(objects_4_classes[1].as_string().unwrap(), "NSIndexSet");
        assert_eq!(objects_4_classes[2].as_string().unwrap(), "NSObject");
        assert_eq!(
            objects_4.get("$classname").unwrap().as_string().unwrap(),
            "NSMutableIndexSet"
        );

        let top = dict.get("$top").unwrap().as_dictionary().unwrap();
        assert_eq!(
            top.get("foundItems")
                .unwrap()
                .as_unsigned_integer()
                .unwrap(),
            1
        );

        let version = dict.get("$version").unwrap().as_unsigned_integer().unwrap();
        assert_eq!(version, 100000);
    }

    #[test]
    fn dictionary_deserialize_dictionary_in_struct() {
        // Example from <https://github.com/ebarnard/rust-plist/issues/54>
        #[derive(Deserialize)]
        struct LayerinfoData {
            color: Option<String>,
            lib: Option<Dictionary>,
        }

        let lib_dict: LayerinfoData = crate::from_bytes(r#"
        <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
      <dict>
        <key>color</key>
        <string>1,0.75,0,0.7</string>
        <key>lib</key>
        <dict>
          <key>com.typemytype.robofont.segmentType</key>
          <string>curve</string>
        </dict>
      </dict>
    </plist>
    "#.as_bytes()).unwrap();

        assert_eq!(lib_dict.color.unwrap(), "1,0.75,0,0.7");
        assert_eq!(
            lib_dict
                .lib
                .unwrap()
                .get("com.typemytype.robofont.segmentType")
                .unwrap()
                .as_string()
                .unwrap(),
            "curve"
        );
    }

    #[test]
    fn dictionary_serialize_xml() {
        // Dictionary to be embedded in dict, below.
        let mut inner_dict = Dictionary::new();
        inner_dict.insert(
            "FirstKey".to_owned(),
            Value::String("FirstValue".to_owned()),
        );
        inner_dict.insert("SecondKey".to_owned(), Value::Data(vec![10, 20, 30, 40]));
        inner_dict.insert("ThirdKey".to_owned(), Value::Real(1.234));
        inner_dict.insert(
            "FourthKey".to_owned(),
            Value::Date(Date::from_rfc3339("1981-05-16T11:32:06Z").unwrap()),
        );

        // Top-level dictionary.
        let mut dict = Dictionary::new();
        dict.insert(
            "AnArray".to_owned(),
            Value::Array(vec![
                Value::String("Hello, world!".to_owned()),
                Value::Integer(Integer::from(345)),
            ]),
        );
        dict.insert("ADictionary".to_owned(), Value::Dictionary(inner_dict));
        dict.insert("AnInteger".to_owned(), Value::Integer(Integer::from(123)));
        dict.insert("ATrueBoolean".to_owned(), Value::Boolean(true));
        dict.insert("AFalseBoolean".to_owned(), Value::Boolean(false));

        // Serialize dictionary as an XML plist.
        let mut buf = Cursor::new(Vec::new());
        crate::to_writer_xml(&mut buf, &dict).unwrap();
        let buf = buf.into_inner();
        let xml = std::str::from_utf8(&buf).unwrap();

        let comparison = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
\t<key>AnArray</key>
\t<array>
\t\t<string>Hello, world!</string>
\t\t<integer>345</integer>
\t</array>
\t<key>ADictionary</key>
\t<dict>
\t\t<key>FirstKey</key>
\t\t<string>FirstValue</string>
\t\t<key>SecondKey</key>
\t\t<data>\n\t\tChQeKA==\n\t\t</data>
\t\t<key>ThirdKey</key>
\t\t<real>1.234</real>
\t\t<key>FourthKey</key>
\t\t<date>1981-05-16T11:32:06Z</date>
\t</dict>
\t<key>AnInteger</key>
\t<integer>123</integer>
\t<key>ATrueBoolean</key>
\t<true/>
\t<key>AFalseBoolean</key>
\t<false/>
</dict>
</plist>";

        assert_eq!(xml, comparison);
    }
}
