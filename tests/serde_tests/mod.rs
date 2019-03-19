use plist::stream::{Event, VecWriter};
use plist::{Date, Deserializer, Error, Serializer};
use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use std::fmt::Debug;
use std::time::SystemTime;

fn new_serializer() -> Serializer<VecWriter> {
    Serializer::new(VecWriter::new())
}

fn new_deserializer(events: Vec<Event>) -> Deserializer<Vec<Result<Event, Error>>> {
    let result_events = events.into_iter().map(Ok).collect();
    Deserializer::new(result_events)
}

fn assert_roundtrip<T>(obj: T, comparison: Option<&[Event]>)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    let mut se = new_serializer();

    obj.serialize(&mut se).unwrap();

    let events = se.into_inner().into_inner();

    if let Some(comparison) = comparison {
        assert_eq!(&events[..], comparison);
    }

    let mut de = new_deserializer(events);

    let new_obj = T::deserialize(&mut de).unwrap();

    assert_eq!(new_obj, obj);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Animal {
    Cow,
    Dog(DogOuter),
    Frog(Result<String, bool>, Vec<f64>),
    Cat {
        age: usize,
        name: String,
        firmware: Option<Vec<u8>>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct DogOuter {
    inner: Vec<DogInner>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct DogInner {
    a: (),
    b: usize,
    c: Vec<String>,
}

#[test]
fn cow() {
    let cow = Animal::Cow;

    let comparison = &[
        Event::StartDictionary(Some(1)),
        Event::String("Cow".to_owned()),
        Event::String("".to_owned()),
        Event::EndDictionary,
    ];

    assert_roundtrip(cow, Some(comparison));
}

#[test]
fn dog() {
    let dog = Animal::Dog(DogOuter {
        inner: vec![DogInner {
            a: (),
            b: 12,
            c: vec!["a".to_string(), "b".to_string()],
        }],
    });

    let comparison = &[
        Event::StartDictionary(Some(1)),
        Event::String("Dog".to_owned()),
        Event::StartDictionary(None),
        Event::String("inner".to_owned()),
        Event::StartArray(Some(1)),
        Event::StartDictionary(None),
        Event::String("a".to_owned()),
        Event::String("".to_owned()),
        Event::String("b".to_owned()),
        Event::Integer(12.into()),
        Event::String("c".to_owned()),
        Event::StartArray(Some(2)),
        Event::String("a".to_owned()),
        Event::String("b".to_owned()),
        Event::EndArray,
        Event::EndDictionary,
        Event::EndArray,
        Event::EndDictionary,
        Event::EndDictionary,
    ];

    assert_roundtrip(dog, Some(comparison));
}

#[test]
fn frog() {
    let frog = Animal::Frog(
        Ok("hello".to_owned()),
        vec![1.0, 2.0, 3.14159, 0.000000001, 1.27e31],
    );

    let comparison = &[
        Event::StartDictionary(Some(1)),
        Event::String("Frog".to_owned()),
        Event::StartArray(Some(2)),
        Event::StartDictionary(Some(1)),
        Event::String("Ok".to_owned()),
        Event::String("hello".to_owned()),
        Event::EndDictionary,
        Event::StartArray(Some(5)),
        Event::Real(1.0),
        Event::Real(2.0),
        Event::Real(3.14159),
        Event::Real(0.000000001),
        Event::Real(1.27e31),
        Event::EndArray,
        Event::EndArray,
        Event::EndDictionary,
    ];

    assert_roundtrip(frog, Some(comparison));
}

#[test]
fn cat() {
    let cat = Animal::Cat {
        age: 12,
        name: "Paws".to_owned(),
        firmware: Some(vec![0, 1, 2, 3, 4, 5, 6, 7, 8]),
    };

    let comparison = &[
        Event::StartDictionary(Some(1)),
        Event::String("Cat".to_owned()),
        Event::StartDictionary(None),
        Event::String("age".to_owned()),
        Event::Integer(12.into()),
        Event::String("name".to_owned()),
        Event::String("Paws".to_owned()),
        Event::String("firmware".to_owned()),
        Event::StartArray(Some(9)),
        Event::Integer(0.into()),
        Event::Integer(1.into()),
        Event::Integer(2.into()),
        Event::Integer(3.into()),
        Event::Integer(4.into()),
        Event::Integer(5.into()),
        Event::Integer(6.into()),
        Event::Integer(7.into()),
        Event::Integer(8.into()),
        Event::EndArray,
        Event::EndDictionary,
        Event::EndDictionary,
    ];

    assert_roundtrip(cat, Some(comparison));
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct NewtypeStruct(NewtypeInner);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct NewtypeInner(u8, u8, u8);

#[test]
fn newtype_struct() {
    let newtype = NewtypeStruct(NewtypeInner(34, 32, 13));

    let comparison = &[
        Event::StartArray(Some(3)),
        Event::Integer(34.into()),
        Event::Integer(32.into()),
        Event::Integer(13.into()),
        Event::EndArray,
    ];

    assert_roundtrip(newtype, Some(comparison));
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TypeWithOptions {
    a: Option<String>,
    b: Option<u32>,
    c: Option<Box<TypeWithOptions>>,
}

#[test]
fn type_with_options() {
    let inner = TypeWithOptions {
        a: None,
        b: Some(12),
        c: None,
    };

    let obj = TypeWithOptions {
        a: Some("hello".to_owned()),
        b: None,
        c: Some(Box::new(inner)),
    };

    let comparison = &[
        Event::StartDictionary(None),
        Event::String("a".to_owned()),
        Event::String("hello".to_owned()),
        Event::String("c".to_owned()),
        Event::StartDictionary(None),
        Event::String("b".to_owned()),
        Event::Integer(12.into()),
        Event::EndDictionary,
        Event::EndDictionary,
    ];

    assert_roundtrip(obj, Some(comparison));
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TypeWithDate {
    a: Option<i32>,
    b: Option<Date>,
}

#[test]
fn type_with_date() {
    let date: Date = SystemTime::now().into();

    let obj = TypeWithDate {
        a: Some(28),
        b: Some(date.clone()),
    };

    let comparison = &[
        Event::StartDictionary(None),
        Event::String("a".to_owned()),
        Event::Integer(28.into()),
        Event::String("b".to_owned()),
        Event::Date(date),
        Event::EndDictionary,
    ];

    assert_roundtrip(obj, Some(comparison));
}
