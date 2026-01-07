use super::{AtomicValue, XdmItem, XdmValue};
use indexmap::IndexMap;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct XdmMap<N> {
    entries: IndexMap<AtomicValue, XdmValue<N>>,
}

impl<N: Clone> XdmMap<N> {
    pub fn new() -> Self {
        Self {
            entries: IndexMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: IndexMap::with_capacity(capacity),
        }
    }

    pub fn from_entries(entries: Vec<(AtomicValue, XdmValue<N>)>) -> Self {
        let mut map = Self::with_capacity(entries.len());
        for (key, value) in entries {
            map.entries.insert(key, value);
        }
        map
    }

    pub fn get(&self, key: &AtomicValue) -> Option<&XdmValue<N>> {
        self.entries.get(key)
    }

    pub fn contains_key(&self, key: &AtomicValue) -> bool {
        self.entries.contains_key(key)
    }

    pub fn put(&self, key: AtomicValue, value: XdmValue<N>) -> Self {
        let mut new_map = self.clone();
        new_map.entries.insert(key, value);
        new_map
    }

    pub fn remove(&self, key: &AtomicValue) -> Self {
        let mut new_map = self.clone();
        new_map.entries.shift_remove(key);
        new_map
    }

    pub fn keys(&self) -> impl Iterator<Item = &AtomicValue> {
        self.entries.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &XdmValue<N>> {
        self.entries.values()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&AtomicValue, &XdmValue<N>)> {
        self.entries.iter()
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn merge(&self, other: &XdmMap<N>) -> Self {
        let mut result = self.clone();
        for (k, v) in other.entries.iter() {
            result.entries.insert(k.clone(), v.clone());
        }
        result
    }

    pub fn into_items(self) -> Vec<XdmItem<N>> {
        vec![XdmItem::Map(self)]
    }
}

impl<N: Clone> Default for XdmMap<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: PartialEq + Clone> PartialEq for XdmMap<N> {
    fn eq(&self, other: &Self) -> bool {
        if self.entries.len() != other.entries.len() {
            return false;
        }
        self.entries
            .iter()
            .all(|(k, v)| other.entries.get(k).is_some_and(|other_v| v == other_v))
    }
}

impl<N: Eq + Clone> Eq for XdmMap<N> {}

impl<N: Hash + Clone> Hash for XdmMap<N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entries.len().hash(state);
        for (key, value) in &self.entries {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl<N: fmt::Debug> fmt::Display for XdmMap<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "map {{ ")?;
        let mut first = true;
        for (k, v) in &self.entries {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}: {:?}", k, v)?;
            first = false;
        }
        write!(f, " }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_operations() {
        let map: XdmMap<()> = XdmMap::new();
        assert!(map.is_empty());
        assert_eq!(map.size(), 0);

        let map = map.put(
            AtomicValue::String("a".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(1)),
        );
        assert_eq!(map.size(), 1);
        assert!(map.contains_key(&AtomicValue::String("a".to_string())));

        let map = map.put(
            AtomicValue::String("b".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(2)),
        );
        assert_eq!(map.size(), 2);

        let val = map.get(&AtomicValue::String("a".to_string()));
        assert!(val.is_some());
    }

    #[test]
    fn test_map_merge() {
        let map1: XdmMap<()> = XdmMap::from_entries(vec![(
            AtomicValue::String("a".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(1)),
        )]);
        let map2: XdmMap<()> = XdmMap::from_entries(vec![(
            AtomicValue::String("b".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(2)),
        )]);

        let merged = map1.merge(&map2);
        assert_eq!(merged.size(), 2);
        assert!(merged.contains_key(&AtomicValue::String("a".to_string())));
        assert!(merged.contains_key(&AtomicValue::String("b".to_string())));
    }

    #[test]
    fn test_map_remove() {
        let map: XdmMap<()> = XdmMap::from_entries(vec![
            (
                AtomicValue::String("a".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(1)),
            ),
            (
                AtomicValue::String("b".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(2)),
            ),
        ]);

        let map = map.remove(&AtomicValue::String("a".to_string()));
        assert_eq!(map.size(), 1);
        assert!(!map.contains_key(&AtomicValue::String("a".to_string())));
        assert!(map.contains_key(&AtomicValue::String("b".to_string())));
    }

    #[test]
    fn test_map_equality_compares_values() {
        let map1: XdmMap<()> = XdmMap::from_entries(vec![
            (
                AtomicValue::String("a".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(1)),
            ),
            (
                AtomicValue::String("b".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(2)),
            ),
        ]);
        let map2: XdmMap<()> = XdmMap::from_entries(vec![
            (
                AtomicValue::String("a".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(1)),
            ),
            (
                AtomicValue::String("b".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(2)),
            ),
        ]);
        let map3: XdmMap<()> = XdmMap::from_entries(vec![
            (
                AtomicValue::String("a".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(1)),
            ),
            (
                AtomicValue::String("b".to_string()),
                XdmValue::from_atomic(AtomicValue::Integer(99)),
            ),
        ]);

        // Same keys AND values should be equal
        assert_eq!(map1, map2);

        // Same keys but different values should NOT be equal
        assert_ne!(map1, map3);
    }

    #[test]
    fn test_map_hash_includes_values() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_map(map: &XdmMap<()>) -> u64 {
            let mut hasher = DefaultHasher::new();
            map.hash(&mut hasher);
            hasher.finish()
        }

        let map1: XdmMap<()> = XdmMap::from_entries(vec![(
            AtomicValue::String("a".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(1)),
        )]);
        let map2: XdmMap<()> = XdmMap::from_entries(vec![(
            AtomicValue::String("a".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(1)),
        )]);
        let map3: XdmMap<()> = XdmMap::from_entries(vec![(
            AtomicValue::String("a".to_string()),
            XdmValue::from_atomic(AtomicValue::Integer(99)), // Different value
        )]);

        // Same keys AND values should have same hash
        assert_eq!(hash_map(&map1), hash_map(&map2));

        // Same keys but different values should have different hash
        assert_ne!(hash_map(&map1), hash_map(&map3));
    }
}
